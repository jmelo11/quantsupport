use crate::{
    scripting::{
        nodes::node::Node,
        utils::errors::{Result, ScriptingError},
    },
    time::date::Date,
};
use serde::{Deserialize, Serialize};

/// # CodedEvent
/// A coded event is a combination of a reference date and a coded expression. Its a precompiled version of an event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodedEvent {
    event_date: Date,
    script: String,
}

impl CodedEvent {
    pub fn new(event_date: Date, script: String) -> CodedEvent {
        CodedEvent { event_date, script }
    }

    pub fn event_date(&self) -> Date {
        self.event_date
    }

    pub fn script(&self) -> &String {
        &self.script
    }
}

/// # Event
/// An event is a combination of a reference date and an expression tree. Represents a future action that will happen at a specific date.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    event_date: Date,
    expr: Node,
}

impl Event {
    pub fn new(event_date: Date, expr: Node) -> Event {
        Event { event_date, expr }
    }

    pub fn event_date(&self) -> Date {
        self.event_date
    }

    pub fn expr(&self) -> &Node {
        &self.expr
    }

    pub fn mut_expr(&mut self) -> &mut Node {
        &mut self.expr
    }
}

impl TryFrom<CodedEvent> for Event {
    type Error = ScriptingError;

    fn try_from(event: CodedEvent) -> Result<Event> {
        let expr = match Node::try_from(event.script().clone()) {
            Ok(expr) => expr,
            Err(e) => {
                return Err(ScriptingError::InvalidSyntax(format!(
                    "{} (event date: {})",
                    e,
                    event.event_date()
                )));
            }
        };
        Ok(Event::new(event.event_date(), expr))
    }
}

/// # EventStream
/// An event stream is a collection of events that will happen in the future. An event stream could represent a series of cash flows, for example.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventStream {
    id: Option<usize>,
    events: Vec<Event>,
}

impl EventStream {
    pub fn new() -> EventStream {
        EventStream {
            events: Vec::new(),
            id: None,
        }
    }

    pub fn with_id(mut self, id: usize) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_events(mut self, events: Vec<Event>) -> Self {
        self.events = events;
        self
    }

    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn events(&self) -> &Vec<Event> {
        &self.events
    }

    pub fn mut_events(&mut self) -> &mut Vec<Event> {
        &mut self.events
    }

    pub fn event_dates(&self) -> Vec<Date> {
        self.events.iter().map(|e| e.event_date).collect()
    }
}

impl TryFrom<Vec<CodedEvent>> for EventStream {
    type Error = ScriptingError;

    fn try_from(events: Vec<CodedEvent>) -> Result<EventStream> {
        let mut event_stream = EventStream::new();
        events.iter().try_for_each(|event| -> Result<()> {
            let event = Event::try_from(event.clone())?;
            event_stream.add_event(event);
            Ok(())
        })?;
        Ok(event_stream)
    }
}
