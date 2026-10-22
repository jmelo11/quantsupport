//! Main-thread ownership of worksheet objects. Maintenance never calculates.

use super::registry::{self, CellId};
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
struct Workbook {
    name: String,
    // COM identities are numeric tokens only; no Workbook/Range references survive
    // a callback, so the cache cannot keep a closed workbook alive.
    com_identity: Option<usize>,
    sheets: HashMap<usize, String>,
    closing: bool,
}

#[derive(Default)]
struct Sessions {
    next_id: u64,
    books: HashMap<u64, Workbook>,
}

impl Sessions {
    fn caller(&mut self, name: &str, sheet: usize, sheet_name: &str) -> u64 {
        let existing = self.books.iter().find_map(|(id, book)| {
            (book.name.eq_ignore_ascii_case(name)
                || (!book.closing && book.sheets.contains_key(&sheet)))
            .then_some(*id)
        });
        let id = existing.unwrap_or_else(|| {
            self.next_id += 1;
            self.next_id
        });
        let book = self.books.entry(id).or_default();
        if book.name != name || book.sheets.get(&sheet).map(String::as_str) != Some(sheet_name) {
            registry::browser_changed();
        }
        book.name = name.to_string();
        book.sheets.insert(sheet, sheet_name.to_string());
        id
    }

    fn reconcile(&mut self, open: &[(usize, String)]) -> Vec<u64> {
        let mut closed = Vec::new();
        for (id, book) in &mut self.books {
            let live = match book.com_identity {
                Some(identity) => open.iter().find(|(key, _)| *key == identity),
                None => open
                    .iter()
                    .find(|(_, name)| book.name.eq_ignore_ascii_case(name)),
            };
            if let Some((identity, name)) = live {
                book.com_identity = Some(*identity);
                book.name.clone_from(name);
                book.closing = false;
            } else {
                closed.push(*id);
            }
        }
        for id in &closed {
            self.books.remove(id);
        }
        closed
    }

    fn before_close(&mut self, name: &str, identity: usize) {
        for book in self
            .books
            .values_mut()
            .filter(|book| book.name.eq_ignore_ascii_case(name))
        {
            book.com_identity = Some(identity);
            book.closing = true;
        }
    }

    fn opened(&mut self, name: &str, identity: usize) -> Vec<u64> {
        let retired: Vec<_> = self
            .books
            .iter()
            .filter_map(|(id, book)| {
                (book.name.eq_ignore_ascii_case(name)
                    && (book.closing
                        || book
                            .com_identity
                            .is_some_and(|previous| previous != identity)))
                .then_some(*id)
            })
            .collect();
        for id in &retired {
            self.books.remove(id);
        }
        retired
    }
}

thread_local! {
    static SESSIONS: RefCell<Sessions> = RefCell::new(Sessions::default());
    static IN_CALL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Looks up a workbook by name without changing worksheet ownership or caller state.
pub fn browser_snapshot(name: &str) -> super::browser::BrowserSnapshot {
    let mut snapshot = super::browser::BrowserSnapshot {
        workbook: name.into(),
        ..Default::default()
    };
    SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        if let Some((id, book)) = sessions
            .books
            .iter()
            .find(|(_, book)| book.name.eq_ignore_ascii_case(name))
        {
            snapshot.objects = registry::workbook_browser_objects(*id);
            for object in &mut snapshot.objects {
                for cell in &mut object.locations {
                    cell.sheet = book
                        .sheets
                        .get(&(cell.sheet_id as usize))
                        .cloned()
                        .unwrap_or_default();
                }
            }
        }
    });
    snapshot
}

pub fn browser_query(
    request: &super::browser_queries::BrowserQuery,
) -> super::browser_queries::BrowserResponse {
    SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        match sessions
            .books
            .iter()
            .find(|(_, book)| book.name.eq_ignore_ascii_case(&request.workbook))
        {
            Some((id, _)) => registry::workbook_browser_query(*id, request),
            None => super::browser_queries::BrowserResponse::error(
                "The workbook has no live objects. Recalculate and refresh.",
            ),
        }
    })
}

/// Clears all workbook objects and AD storage on the calculation thread.
/// Session IDs keep increasing so cached worksheet handles cannot become live again.
pub fn reset_memory() -> Result<usize, String> {
    if IN_CALL.with(std::cell::Cell::get) || registry::calculation_active() {
        return Err("Wait for worksheet calculation to finish before resetting memory.".into());
    }
    let count = registry::live_object_count(true);
    registry::clear_workbooks();
    SESSIONS.with(|sessions| sessions.borrow_mut().books.clear());
    registry::select_workbook(0);
    Ok(count)
}

/// One stateful UDF invocation. Early returns and unwinding release old outputs.
pub struct CallScope {
    previous: Option<u64>,
}

impl CallScope {
    pub fn enter() -> Self {
        if IN_CALL.with(std::cell::Cell::get) {
            return Self { previous: None };
        }
        #[cfg(windows)]
        if let Some((name, sheet_name, cell, formula)) = windows::caller() {
            let workbook = SESSIONS
                .with(|sessions| sessions.borrow_mut().caller(&name, cell.sheet, &sheet_name));
            IN_CALL.with(|active| active.set(true));
            let previous = registry::select_workbook(workbook);
            registry::with_registry_mut(|registry| {
                registry.begin_cell(cell);
                registry.set_cell_formula(cell, formula);
            });
            return Self {
                previous: Some(previous),
            };
        }
        Self { previous: None }
    }

    #[cfg_attr(test, allow(dead_code))]
    pub fn has_caller(&self) -> bool {
        self.previous.is_some() || IN_CALL.with(std::cell::Cell::get)
    }

    pub fn finish(mut self, success: bool) {
        self.complete(success);
    }

    fn complete(&mut self, success: bool) {
        if let Some(previous) = self.previous.take() {
            // Some native engines leave recording active on an error path.
            // No worksheet constructor may accidentally record persistent inputs.
            quantsupport::prelude::Tape::stop_recording_fwd();
            quantsupport::prelude::Tape::stop_recording();
            registry::with_registry_mut(|registry| registry.finish_cell(success));
            registry::select_workbook(previous);
            IN_CALL.with(|active| active.set(false));
        }
    }
}

impl Drop for CallScope {
    fn drop(&mut self) {
        self.complete(false);
    }
}

#[cfg(windows)]
pub mod windows {
    use super::*;
    use ::windows::{
        core::{IUnknown, IUnknown_Vtbl, Interface, BSTR, GUID, HRESULT, PCWSTR},
        Win32::{
            Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
            System::{
                Com::{
                    IConnectionPoint, IConnectionPointContainer, IDispatch, IDispatch_Vtbl,
                    DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPPARAMS, EXCEPINFO,
                },
                LibraryLoader::{
                    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                },
                Threading::GetCurrentProcessId,
                Variant::{VARIANT, VT_ERROR},
            },
            UI::{
                Accessibility::AccessibleObjectFromWindow,
                WindowsAndMessaging::{
                    CreateWindowExW, DefWindowProcW, DestroyWindow, EnumChildWindows, EnumWindows,
                    GetClassNameW, GetWindowThreadProcessId, KillTimer, RegisterClassW, SetTimer,
                    UnregisterClassW, HWND_MESSAGE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_TIMER,
                    WNDCLASSW,
                },
            },
        },
    };
    use std::{
        ffi::c_void,
        ptr,
        sync::atomic::{AtomicU32, Ordering},
    };
    use xll_rs::{
        entrypoint::{excel12, excel_free},
        types::{XLTYPE_REF, XLTYPE_SREF, XL_SHEET_ID, XL_SHEET_NM},
    };

    const APP_EVENTS: GUID = GUID::from_u128(0x00024413_0000_0000_c000_000000000046);
    const SHEET_CHANGE: i32 = 0x61c;
    const XLF_CALLER: i32 = 89;
    const XLF_FORMULATEXT: i32 = 588;
    const E_NOINTERFACE: HRESULT = HRESULT(0x80004002_u32 as i32);
    const E_NOTIMPL: HRESULT = HRESULT(0x80004001_u32 as i32);
    const S_OK: HRESULT = HRESULT(0);

    struct Subscription {
        app: IDispatch,
        point: IConnectionPoint,
        cookie: u32,
    }
    impl Drop for Subscription {
        fn drop(&mut self) {
            let _ = unsafe { self.point.Unadvise(self.cookie) };
        }
    }
    thread_local! {
        static TIMER: RefCell<Option<(HWND, HMODULE)>> = const { RefCell::new(None) };
        static SUBSCRIPTION: RefCell<Option<Subscription>> = const { RefCell::new(None) };
        static BUSY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static RESTORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        static LAST_BOOK_COUNT: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
        static AUDIT_TICKS: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    }

    #[cfg(test)]
    thread_local! {
        static TEST_CALLER: RefCell<Option<(String, String, CellId, Option<String>)>> = const { RefCell::new(None) };
    }

    #[cfg(test)]
    pub(crate) fn set_test_caller(caller: Option<(String, String, CellId, Option<String>)>) {
        TEST_CALLER.with(|value| *value.borrow_mut() = caller);
    }

    pub fn caller() -> Option<(String, String, CellId, Option<String>)> {
        #[cfg(test)]
        if let Some(caller) = TEST_CALLER.with(|value| value.borrow().clone()) {
            return Some(caller);
        }

        let (status, mut caller) = excel12(XLF_CALLER, &mut []);
        if status != 0 {
            excel_free(&mut caller);
            return None;
        }
        let parsed = match caller.base_type() {
            XLTYPE_REF => {
                let reference = unsafe { &caller.val.mref };
                if reference.lpmref.is_null() {
                    None
                } else {
                    Some((
                        unsafe { (*reference.lpmref).reftbl[0] },
                        Some(reference.id_sheet),
                    ))
                }
            }
            XLTYPE_SREF => Some((unsafe { caller.val.sref.ref_ }, None)),
            _ => None,
        };
        let result = parsed.and_then(|(range, sheet_id)| {
            let (status, mut sheet) = excel12(XL_SHEET_NM, &mut [&mut caller]);
            let name = (status == 0).then(|| sheet.as_string()).flatten();
            // A local reference has no sheet ID. Resolve it from the caller's
            // qualified sheet name, never from Excel's active/front sheet.
            let sheet_id = sheet_id.or_else(|| {
                let (status, mut id) = excel12(XL_SHEET_ID, &mut [&mut sheet]);
                let result = (status == 0 && id.base_type() == XLTYPE_REF)
                    .then(|| unsafe { id.val.mref.id_sheet });
                excel_free(&mut id);
                result
            });
            excel_free(&mut sheet);
            let name = name?;
            let start = name.rfind('[')?;
            let end = name.rfind(']')?;
            let cell = CellId {
                sheet: sheet_id?,
                row: range.rw_first,
                column: range.col_first,
            };
            let (status, mut formula) = excel12(XLF_FORMULATEXT, &mut [&mut caller]);
            let text = (status == 0).then(|| formula.as_string()).flatten();
            excel_free(&mut formula);
            Some((
                name[start + 1..end].to_string(),
                name[end + 1..].to_string(),
                cell,
                text,
            ))
        });
        excel_free(&mut caller);
        result
    }

    fn member(
        object: &IDispatch,
        name: &str,
        args: &mut [VARIANT],
        flags: DISPATCH_FLAGS,
    ) -> ::windows::core::Result<VARIANT> {
        let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
        let mut id = 0;
        let mut result = VARIANT::default();
        let params = DISPPARAMS {
            rgvarg: args.as_mut_ptr(),
            cArgs: args.len() as u32,
            ..Default::default()
        };
        unsafe {
            object.GetIDsOfNames(&GUID::zeroed(), &PCWSTR(wide.as_ptr()), 1, 0, &mut id)?;
            object.Invoke(
                id,
                &GUID::zeroed(),
                0,
                flags,
                &params,
                Some(&mut result),
                None,
                None,
            )?;
        }
        Ok(result)
    }
    fn get(
        object: &IDispatch,
        name: &str,
        args: &mut [VARIANT],
    ) -> ::windows::core::Result<VARIANT> {
        member(object, name, args, DISPATCH_PROPERTYGET)
    }

    fn missing() -> VARIANT {
        let mut value = VARIANT::default();
        unsafe {
            (*value.Anonymous.Anonymous).vt = VT_ERROR;
            (*value.Anonymous.Anonymous).Anonymous.scode = 0x80020004_u32 as i32;
        }
        value
    }

    fn contains_qs(
        range: &IDispatch,
        pattern: &str,
        look_in: i32,
    ) -> ::windows::core::Result<bool> {
        let mut args = [
            false.into(), // SearchFormat: ignore persisted Find-dialog formatting.
            missing(),    // MatchByte
            false.into(),
            1.into(),
            1.into(),
            2.into(),
            look_in.into(),
            missing(),
            pattern.into(),
        ];
        Ok(IDispatch::try_from(&member(range, "Find", &mut args, DISPATCH_METHOD)?).is_ok())
    }

    fn restore_workbook(book: &IDispatch) -> ::windows::core::Result<()> {
        let sheets = dispatch(book, "Worksheets", &mut [])?;
        for position in 1..=integer(&sheets, "Count")? {
            let sheet = dispatch(&sheets, "Item", &mut [position.into()])?;
            let used = dispatch(&sheet, "UsedRange", &mut [])?;
            if contains_qs(&used, "QS.", -4123)? || contains_qs(&used, "QSObject:", -4163)? {
                // Dirty follows Excel's calculation mode: Automatic reconstructs
                // handles; Manual defers this until the user calculates.
                member(&used, "Dirty", &mut [], DISPATCH_METHOD)?;
            }
        }
        Ok(())
    }

    fn restore_open_workbooks(app: &IDispatch) -> ::windows::core::Result<()> {
        let books = dispatch(app, "Workbooks", &mut [])?;
        for position in 1..=integer(&books, "Count")? {
            restore_workbook(&dispatch(&books, "Item", &mut [position.into()])?)?;
        }
        Ok(())
    }

    fn dispatch(
        object: &IDispatch,
        name: &str,
        args: &mut [VARIANT],
    ) -> ::windows::core::Result<IDispatch> {
        IDispatch::try_from(&get(object, name, args)?)
    }
    fn text(object: &IDispatch, name: &str) -> ::windows::core::Result<String> {
        Ok(BSTR::try_from(&get(object, name, &mut [])?)?.to_string())
    }
    fn integer(object: &IDispatch, name: &str) -> ::windows::core::Result<i32> {
        i32::try_from(&get(object, name, &mut [])?)
    }

    // Locate the native object model in this Excel process, never the ROT's
    // potentially different Excel instance. Called only on Excel's UI thread.
    unsafe extern "system" fn child(hwnd: HWND, data: LPARAM) -> ::windows::core::BOOL {
        let mut class = [0u16; 64];
        let length = unsafe { GetClassNameW(hwnd, &mut class) };
        if String::from_utf16_lossy(&class[..length as usize]) == "EXCEL7" {
            let mut raw = ptr::null_mut();
            if unsafe { AccessibleObjectFromWindow(hwnd, 0xfffffff0, &IDispatch::IID, &mut raw) }
                .is_ok()
                && !raw.is_null()
            {
                let window = unsafe { IDispatch::from_raw(raw) };
                if let Ok(app) = dispatch(&window, "Application", &mut []) {
                    unsafe {
                        *(data.0 as *mut Option<IDispatch>) = Some(app);
                    }
                    return false.into();
                }
            }
        }
        true.into()
    }
    unsafe extern "system" fn top(hwnd: HWND, data: LPARAM) -> ::windows::core::BOOL {
        let mut process = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process));
        }
        if process == unsafe { GetCurrentProcessId() } {
            let _ = unsafe { EnumChildWindows(Some(hwnd), Some(child), data) };
        }
        unsafe { (*(data.0 as *mut Option<IDispatch>)).is_none() }.into()
    }
    fn application() -> Option<IDispatch> {
        // Keep only Application, never a workbook or range. Its object model is
        // still available after the last workbook window has disappeared.
        if let Some(app) = SUBSCRIPTION.with(|subscription| {
            subscription
                .borrow()
                .as_ref()
                .map(|value| value.app.clone())
        }) {
            return Some(app);
        }
        let mut app = None;
        let _ = unsafe {
            EnumWindows(
                Some(top),
                LPARAM((&mut app as *mut Option<IDispatch>) as isize),
            )
        };
        app
    }

    fn reconcile(app: &IDispatch) -> ::windows::core::Result<()> {
        let books = dispatch(app, "Workbooks", &mut [])?;
        let count = integer(&books, "Count")?;
        let mut open = Vec::new();
        for position in 1..=count {
            let book = dispatch(&books, "Item", &mut [VARIANT::from(position)])?;
            let identity = book.cast::<IUnknown>()?.as_raw() as usize;
            open.push((identity, text(&book, "Name")?));
        }
        // Commit only after a complete successful enumeration. COM rejection
        // while Excel is busy must never be interpreted as a closed workbook.
        let closed = SESSIONS.with(|sessions| sessions.borrow_mut().reconcile(&open));
        for book in closed {
            registry::close_workbook(book);
        }
        LAST_BOOK_COUNT.with(|previous| previous.set(Some(count)));
        Ok(())
    }

    fn sheet_change(params: *const DISPPARAMS) -> ::windows::core::Result<()> {
        if SESSIONS.with(|sessions| sessions.borrow().books.is_empty()) {
            return Ok(());
        }
        if params.is_null() {
            return Ok(());
        }
        let params = unsafe { &*params };
        if params.cArgs != 2 || params.rgvarg.is_null() {
            return Ok(());
        }
        let args = unsafe { std::slice::from_raw_parts(params.rgvarg, 2) };
        let range = IDispatch::try_from(&args[0])?;
        let sheet = IDispatch::try_from(&args[1])?;
        let book = dispatch(&sheet, "Parent", &mut [])?;
        let book_name = text(&book, "Name")?;
        let sheet_name = text(&sheet, "Name")?;
        let areas = dispatch(&range, "Areas", &mut [])?;
        let mut rectangles = Vec::new();
        for position in 1..=integer(&areas, "Count")? {
            let area = dispatch(&areas, "Item", &mut [position.into()])?;
            let row = integer(&area, "Row")? - 1;
            let column = integer(&area, "Column")? - 1;
            let rows = integer(&dispatch(&area, "Rows", &mut [])?, "Count")?;
            let columns = integer(&dispatch(&area, "Columns", &mut [])?, "Count")?;
            rectangles.push((row, column, row + rows, column + columns));
        }
        let affected = SESSIONS.with(|sessions| {
            sessions
                .borrow()
                .books
                .iter()
                .filter(|(_, book)| book.name.eq_ignore_ascii_case(&book_name))
                .flat_map(|(id, book)| {
                    book.sheets
                        .iter()
                        .filter(|(_, name)| *name == &sheet_name)
                        .map(|(sheet, _)| (*id, *sheet))
                })
                .collect::<Vec<_>>()
        });
        for (book, sheet_id) in affected {
            // Excel may calculate before or after delivering SheetChange.
            // Compare against the actual caller formula recorded by the UDF;
            // never discard an object that the replacement formula just made.
            let cells = registry::workbook_cells_in_rectangles(book, sheet_id, &rectangles);
            if cells.is_empty() {
                continue;
            }
            if bool::try_from(&get(&range, "HasFormula", &mut [])?).ok() == Some(false) {
                // Clearing a large range needs no per-cell COM calls.
                for cell in cells {
                    registry::release_workbook_cell_if_changed(book, cell, "", false);
                }
                continue;
            }
            // Worksheet.Cells is a parameterless property. Its returned Range
            // exposes the indexed Item property for selecting a particular cell.
            let sheet_cells = dispatch(&sheet, "Cells", &mut [])?;
            for cell in cells {
                let target = dispatch(
                    &sheet_cells,
                    "Item",
                    &mut [(cell.column + 1).into(), (cell.row + 1).into()],
                )?;
                let has_formula = bool::try_from(&get(&target, "HasFormula", &mut [])?)?;
                if !has_formula {
                    registry::release_workbook_cell_if_changed(book, cell, "", false);
                    continue;
                }
                let mut first = None;
                let mut matched = false;
                for property in ["Formula2Local", "FormulaLocal", "Formula2", "Formula"] {
                    if let Ok(formula) = text(&target, property) {
                        matched = registry::workbook_cell_formula_matches(book, cell, &formula);
                        first.get_or_insert(formula);
                        if matched {
                            break;
                        }
                    }
                }
                if !matched {
                    if let Some(formula) = first {
                        registry::release_workbook_cell_if_changed(book, cell, &formula, true);
                    }
                }
            }
        }
        Ok(())
    }

    // Excel's outgoing AppEvents dispinterface must be accepted by QueryInterface
    // in addition to IDispatch. No type library or Office installation is needed
    // at compile time.
    #[repr(C)]
    struct Sink {
        vtable: *const IDispatch_Vtbl,
        references: AtomicU32,
    }
    unsafe extern "system" fn query(
        this: *mut c_void,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT {
        if iid.is_null() || out.is_null() {
            return E_NOINTERFACE;
        }
        unsafe {
            *out = ptr::null_mut();
        }
        let iid = unsafe { *iid };
        if iid == APP_EVENTS || iid == IDispatch::IID || iid == IUnknown::IID {
            unsafe {
                *out = this;
                add_ref(this);
            }
            S_OK
        } else {
            E_NOINTERFACE
        }
    }
    unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
        unsafe { &*(this as *mut Sink) }
            .references
            .fetch_add(1, Ordering::Relaxed)
            + 1
    }
    unsafe extern "system" fn release(this: *mut c_void) -> u32 {
        let count = unsafe { &*(this as *mut Sink) }
            .references
            .fetch_sub(1, Ordering::AcqRel)
            - 1;
        if count == 0 {
            drop(unsafe { Box::from_raw(this as *mut Sink) });
        }
        count
    }
    unsafe extern "system" fn type_count(_: *mut c_void, out: *mut u32) -> HRESULT {
        if !out.is_null() {
            unsafe {
                *out = 0;
            }
        }
        S_OK
    }
    unsafe extern "system" fn type_info(
        _: *mut c_void,
        _: u32,
        _: u32,
        _: *mut *mut c_void,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn names(
        _: *mut c_void,
        _: *const GUID,
        _: *const PCWSTR,
        _: u32,
        _: u32,
        _: *mut i32,
    ) -> HRESULT {
        E_NOTIMPL
    }
    unsafe extern "system" fn invoke(
        _: *mut c_void,
        id: i32,
        _: *const GUID,
        _: u32,
        _: ::windows::Win32::System::Com::DISPATCH_FLAGS,
        params: *const DISPPARAMS,
        _: *mut VARIANT,
        _: *mut EXCEPINFO,
        _: *mut u32,
    ) -> HRESULT {
        let _ = std::panic::catch_unwind(|| {
            if id == SHEET_CHANGE {
                let _ = sheet_change(params);
            }
            if id == 0x622 && !params.is_null() {
                // WorkbookBeforeClose(Wb, Cancel)
                let params = unsafe { &*params };
                if params.cArgs == 2 && !params.rgvarg.is_null() {
                    if let Ok(book) = IDispatch::try_from(unsafe { &*params.rgvarg.add(1) }) {
                        if let (Ok(name), Ok(identity)) =
                            (text(&book, "Name"), book.cast::<IUnknown>())
                        {
                            SESSIONS.with(|sessions| {
                                sessions
                                    .borrow_mut()
                                    .before_close(&name, identity.as_raw() as usize)
                            });
                        }
                    }
                }
                LAST_BOOK_COUNT.with(|previous| previous.set(None));
            }
            // NewWorkbook, WorkbookOpen and WorkbookActivate: reconcile before
            // formulas in a reopened workbook can reuse an earlier session.
            if matches!(id, 0x61d | 0x61f | 0x620) && !params.is_null() {
                let params = unsafe { &*params };
                if params.cArgs == 1 && !params.rgvarg.is_null() {
                    if let Ok(book) = IDispatch::try_from(unsafe { &*params.rgvarg }) {
                        if matches!(id, 0x61d | 0x61f) {
                            if let (Ok(name), Ok(identity)) =
                                (text(&book, "Name"), book.cast::<IUnknown>())
                            {
                                let retired = SESSIONS.with(|sessions| {
                                    sessions
                                        .borrow_mut()
                                        .opened(&name, identity.as_raw() as usize)
                                });
                                for id in retired {
                                    registry::close_workbook(id);
                                }
                            }
                        }
                        if let Ok(app) = dispatch(&book, "Application", &mut []) {
                            let _ = reconcile(&app);
                        }
                        if id == 0x61f {
                            let _ = restore_workbook(&book);
                        }
                    }
                }
            }
        });
        S_OK
    }
    static VTABLE: IDispatch_Vtbl = IDispatch_Vtbl {
        base__: IUnknown_Vtbl {
            QueryInterface: query,
            AddRef: add_ref,
            Release: release,
        },
        GetTypeInfoCount: type_count,
        GetTypeInfo: type_info,
        GetIDsOfNames: names,
        Invoke: invoke,
    };

    fn subscribe(app: &IDispatch) -> ::windows::core::Result<Subscription> {
        let container = app.cast::<IConnectionPointContainer>()?;
        let point = unsafe { container.FindConnectionPoint(&APP_EVENTS)? };
        let sink = Box::new(Sink {
            vtable: &VTABLE,
            references: AtomicU32::new(1),
        });
        let sink = unsafe { IUnknown::from_raw(Box::into_raw(sink) as *mut c_void) };
        let cookie = unsafe { point.Advise(&sink)? };
        Ok(Subscription {
            app: app.clone(),
            point,
            cookie,
        })
    }

    unsafe extern "system" fn tick(_: HWND, _: u32, _: usize, _: u32) {
        // UI message pumps can reenter while Excel displays a modal dialog.
        if BUSY.with(|busy| busy.replace(true)) {
            return;
        }
        let _ = std::panic::catch_unwind(|| {
            if let Some(app) = application() {
                // Never compete with a calculation or Excel's cell editor/dialogs.
                if bool::try_from(&get(&app, "Ready", &mut [])?).ok() != Some(true)
                    || integer(&app, "CalculationState")? == 1
                {
                    return Ok::<(), ::windows::core::Error>(());
                }
                if RESTORE.with(|restore| restore.replace(false)) {
                    let _ = restore_open_workbooks(&app);
                }
                SUBSCRIPTION.with(|subscription| {
                    if subscription.borrow().is_none() {
                        if let Ok(value) = subscribe(&app) {
                            *subscription.borrow_mut() = Some(value);
                            RESTORE.with(|restore| restore.set(true));
                        }
                    }
                });
                let count = integer(&dispatch(&app, "Workbooks", &mut [])?, "Count")?;
                let audit = AUDIT_TICKS.with(|ticks| {
                    let previous = ticks.get();
                    ticks.set(if previous == 0 { 30 } else { previous - 1 });
                    previous == 0
                });
                let unidentified = SESSIONS.with(|sessions| {
                    sessions
                        .borrow()
                        .books
                        .values()
                        .any(|book| book.com_identity.is_none())
                });
                if audit
                    || unidentified
                    || LAST_BOOK_COUNT.with(|previous| previous.get() != Some(count))
                {
                    reconcile(&app)?;
                }
            }
            Ok::<(), ::windows::core::Error>(())
        });
        BUSY.with(|busy| busy.set(false));
    }

    const WINDOW_CLASS: PCWSTR = ::windows::core::w!("QuantSupport.ObjectLifetime");

    unsafe extern "system" fn window_proc(
        window: HWND,
        message: u32,
        word: WPARAM,
        long: LPARAM,
    ) -> LRESULT {
        if message == WM_TIMER && word.0 == 1 {
            unsafe {
                tick(window, message, word.0, 0);
            }
            LRESULT(0)
        } else {
            unsafe { DefWindowProcW(window, message, word, long) }
        }
    }

    pub fn start() -> Result<(), String> {
        let mut module = HMODULE::default();
        unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(window_proc as *const () as *const u16),
                &mut module,
            )
        }
        .map_err(|error| error.to_string())?;
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err("could not register the workbook lifecycle window".into());
        }
        let window = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                WINDOW_CLASS,
                ::windows::core::w!(""),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            )
        };
        let window = match window {
            Ok(window) => window,
            Err(error) => {
                let _ = unsafe { UnregisterClassW(WINDOW_CLASS, Some(instance)) };
                return Err(error.to_string());
            }
        };
        if unsafe { SetTimer(Some(window), 1, 1000, None) } == 0 {
            let _ = unsafe { DestroyWindow(window) };
            let _ = unsafe { UnregisterClassW(WINDOW_CLASS, Some(instance)) };
            return Err("could not install the workbook lifecycle timer".into());
        }
        TIMER.with(|timer| *timer.borrow_mut() = Some((window, module)));
        unsafe {
            tick(window, 0, 1, 0);
        }
        Ok(())
    }

    pub fn stop() {
        let timer = TIMER.with(|timer| timer.borrow_mut().take());
        if let Some((window, module)) = timer {
            let _ = unsafe { KillTimer(Some(window), 1) };
            // A window-owned timer uses no queued callback function pointer.
            // Destroying its window is safe even with pending WM_TIMER messages
            // when Excel unloads/reloads this DLL.
            let _ = unsafe { DestroyWindow(window) };
            let _ = unsafe { UnregisterClassW(WINDOW_CLASS, Some(HINSTANCE(module.0))) };
        }
        SUBSCRIPTION.with(|subscription| {
            subscription.borrow_mut().take();
        });
        SESSIONS.with(|sessions| *sessions.borrow_mut() = Sessions::default());
        RESTORE.with(|restore| restore.set(false));
        LAST_BOOK_COUNT.with(|previous| previous.set(None));
        AUDIT_TICKS.with(|ticks| ticks.set(0));
        registry::clear_workbooks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closing_and_reopening_before_an_idle_tick_retires_only_the_old_workbook() {
        let mut sessions = Sessions::default();
        let old = sessions.caller("Market.xlsx", 11, "Inputs");
        let other = sessions.caller("Other.xlsx", 22, "Inputs");
        sessions.before_close("Market.xlsx", 101);
        let retired = sessions.opened("Market.xlsx", 102);
        assert_eq!(retired, vec![old]);
        assert!(sessions.books.contains_key(&other));
        let fresh = sessions.caller("Market.xlsx", 11, "Inputs");
        assert_ne!(fresh, old);
        assert!(sessions
            .reconcile(&[(102, "Market.xlsx".into()), (202, "Other.xlsx".into())])
            .is_empty());
        sessions.before_close("Other.xlsx", 202);
        assert!(sessions
            .reconcile(&[(102, "Market.xlsx".into()), (202, "Other.xlsx".into())])
            .is_empty());
        assert_eq!(
            sessions.caller("Other.xlsx", 22, "Inputs"),
            other,
            "cancelled closes preserve the workbook session"
        );
    }

    #[test]
    fn reset_drops_all_workbooks_and_never_revives_old_handles() {
        use quantsupport::prelude::FixingStore;
        registry::clear_workbooks();
        let make = |name: &str, sheet| {
            let id = SESSIONS.with(|sessions| sessions.borrow_mut().caller(name, sheet, "Inputs"));
            registry::select_workbook(id);
            registry::with_registry_mut(|registry| {
                registry
                    .upsert_if_changed(
                        "fixings",
                        registry::QsObject::FixingStore(Box::new(FixingStore::default())),
                        "empty",
                    )
                    .unwrap()
            })
        };
        let old = make("One.xlsx", 900);
        make("Two.xlsx", 901);
        let version = registry::browser_revision();
        assert_eq!(reset_memory().unwrap(), 2);
        assert!(registry::browser_revision() > version);
        assert_eq!(registry::live_object_count(true), 0);
        assert!(browser_snapshot("One.xlsx").objects.is_empty());
        let fresh = make("One.xlsx", 900);
        assert_ne!(old, fresh);
        registry::with_registry(|registry| assert!(registry.fixing_store(&old).is_err()));
        registry::with_registry_mut(|registry| {
            registry.begin_cell(CellId {
                sheet: 900,
                row: 0,
                column: 0,
            })
        });
        assert!(reset_memory().is_err());
        assert_eq!(registry::live_object_count(true), 1);
        registry::with_registry_mut(|registry| registry.finish_cell(true));
        reset_memory().unwrap();
    }

    #[test]
    fn polling_browser_revision_does_not_visit_or_change_registry_state() {
        registry::clear_workbooks();
        let version = registry::browser_revision();
        for _ in 0..10_000 {
            assert_eq!(registry::browser_revision(), version);
        }
        browser_snapshot("not open.xlsx");
        assert_eq!(registry::browser_revision(), version);
        registry::select_workbook(5000);
        registry::with_registry_mut(|registry| {
            registry
                .upsert_if_changed(
                    "fixings",
                    registry::QsObject::FixingStore(Box::default()),
                    "empty",
                )
                .unwrap();
        });
        assert!(registry::browser_revision() > version);
        let version = registry::browser_revision();
        registry::workbook_browser_objects(5000);
        assert_eq!(registry::browser_revision(), version);
        registry::close_workbook(5000);
        assert!(registry::browser_revision() > version);
        registry::select_workbook(0);
    }

    #[test]
    fn browser_snapshot_matches_the_managed_contract_and_resolves_workbook_names() {
        use quantsupport::prelude::{Date, Level, PricingContext, QuoteStore};
        SESSIONS.with(|sessions| *sessions.borrow_mut() = Sessions::default());
        registry::clear_workbooks();
        let id = SESSIONS.with(|sessions| sessions.borrow_mut().caller("Café.xlsx", 42, "Marché"));
        let previous = registry::select_workbook(id);
        registry::with_registry_mut(|registry| {
            let cell = CellId {
                sheet: 42,
                row: 0,
                column: 0,
            };
            registry.begin_cell(cell);
            registry.set_cell_formula(
                cell,
                Some("=QS.CONTEXT.CREATE(\"café \"\"market\"\"\",B2,B3)".into()),
            );
            let context = PricingContext::new()
                .with_quote_store(QuoteStore::new(Date::new(2026, 1, 15)))
                .with_quote_level(Level::Ask)
                .with_snapshot_timestamp(
                    super::super::historical::timestamp("2026-01-16T00:30:00Z").unwrap(),
                );
            registry
                .upsert_if_changed(
                    "café \"market\"",
                    registry::QsObject::PricingContext(Box::new(context)),
                    "contract",
                )
                .unwrap();
            registry.finish_cell(true);
        });
        registry::select_workbook(previous);
        let snapshot = browser_snapshot("Café.xlsx");
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../ribbon/tests/snapshot.json")).unwrap();
        assert_eq!(serde_json::to_value(&snapshot).unwrap(), fixture);
        assert_eq!(
            browser_snapshot("café.XLSX").objects[0].handle,
            snapshot.objects[0].handle
        );
        assert!(browser_snapshot("Other.xlsx").objects.is_empty());
        registry::close_workbook(id);
        assert!(browser_snapshot("Café.xlsx").objects.is_empty());
        SESSIONS.with(|sessions| *sessions.borrow_mut() = Sessions::default());
    }

    #[test]
    fn workbook_names_are_isolated_and_sheet_identity_survives_save_as() {
        let mut sessions = Sessions::default();
        let a = sessions.caller("Book1.xlsx", 11, "Sheet1");
        let b = sessions.caller("Book2.xlsx", 12, "Sheet1");
        assert_ne!(a, b);
        assert_eq!(sessions.caller("BOOK1.XLSX", 11, "Sheet1"), a);
        assert!(sessions
            .reconcile(&[(101, "Book1.xlsx".into()), (102, "Book2.xlsx".into())])
            .is_empty());
        assert!(sessions
            .reconcile(&[(101, "Renamed.xlsx".into()), (102, "Book2.xlsx".into())])
            .is_empty());
        assert_eq!(sessions.caller("Renamed.xlsx", 11, "Sheet1"), a);
        assert_eq!(sessions.reconcile(&[(102, "Book2.xlsx".into())]), vec![a]);
        assert!(sessions.books.contains_key(&b));
    }

    #[test]
    fn closing_the_last_workbook_and_reopening_its_name_never_reuses_a_session() {
        let mut sessions = Sessions::default();
        let previous = sessions.caller("Market.xlsx", 11, "Inputs");
        assert!(sessions
            .reconcile(&[(101, "Market.xlsx".into())])
            .is_empty());
        assert_eq!(
            sessions.reconcile(&[(102, "Market.xlsx".into())]),
            vec![previous]
        );
        let next = sessions.caller("Market.xlsx", 21, "Inputs");
        assert_ne!(previous, next);
        assert!(sessions
            .reconcile(&[(102, "Market.xlsx".into())])
            .is_empty());
        assert_eq!(sessions.reconcile(&[]), vec![next]);
        assert!(sessions.books.is_empty());
    }
}
