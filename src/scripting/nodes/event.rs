/*
This file is part of QuantSupport's Rust rewrite and adaptation of the
derivatives scripting code written by Antoine Savine in 2018.

The original code is the strict intellectual property of Antoine Savine.

A license to use and alter the original code for personal and commercial
applications is freely granted to any person or company that purchased a copy
of the book:

Modern Computational Finance: Scripting for Derivatives and XVA
Jesper Andreasen and Antoine Savine
Wiley, 2018

This attribution and license notice must be preserved at the top of this file.
*/

use crate::utils::errors::{QSError, Result};

use crate::{scripting::nodes::node::Node, time::date::Date};
use serde::{Deserialize, Serialize};

/// Dated script source, before parsing or market-data indexing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodedEvent {
    event_date: Date,
    script: String,
}

impl CodedEvent {
    /// Creates a dated source-code event.
    pub fn new(event_date: Date, script: String) -> CodedEvent {
        CodedEvent { event_date, script }
    }

    /// Returns the event date.
    pub fn event_date(&self) -> Date {
        self.event_date
    }

    /// Returns the event source code.
    pub fn script(&self) -> &String {
        &self.script
    }
}

/// Dated, parsed expression tree.
///
/// Parsing does not assign market-response indices; compilation by
/// [`ScriptEngine`](crate::scripting::runtime::ScriptEngine) performs that step.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    event_date: Date,
    expr: Node,
}

impl Event {
    /// Creates a dated, parsed expression event.
    pub fn new(event_date: Date, expr: Node) -> Event {
        Event { event_date, expr }
    }

    /// Returns the event date.
    pub fn event_date(&self) -> Date {
        self.event_date
    }

    /// Returns the event expression tree.
    pub fn expr(&self) -> &Node {
        &self.expr
    }

    /// Returns mutable access to the event expression tree.
    pub fn mut_expr(&mut self) -> &mut Node {
        &mut self.expr
    }
}

impl TryFrom<CodedEvent> for Event {
    type Error = QSError;

    fn try_from(event: CodedEvent) -> Result<Event> {
        let expr = Node::try_from(event.script().clone()).map_err(|error| {
            error.context(format!("parsing script event dated {}", event.event_date()))
        })?;
        Ok(Event::new(event.event_date(), expr))
    }
}

/// Script events in their supplied execution order.
///
/// Construction and mutation do not sort events. The script engine checks that
/// dates are nondecreasing and do not precede its reference date.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventStream {
    id: Option<usize>,
    events: Vec<Event>,
}

impl EventStream {
    /// Creates an empty event stream.
    pub fn new() -> EventStream {
        EventStream {
            events: Vec::new(),
            id: None,
        }
    }

    /// Assigns an optional stream identifier.
    pub fn with_id(mut self, id: usize) -> Self {
        self.id = Some(id);
        self
    }

    /// Replaces the events in the stream.
    pub fn with_events(mut self, events: Vec<Event>) -> Self {
        self.events = events;
        self
    }

    /// Appends an event to the stream.
    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Returns the ordered events.
    pub fn events(&self) -> &Vec<Event> {
        &self.events
    }

    /// Returns mutable access to the ordered events.
    pub fn mut_events(&mut self) -> &mut Vec<Event> {
        &mut self.events
    }

    /// Returns event dates in stream order.
    pub fn event_dates(&self) -> Vec<Date> {
        self.events.iter().map(|e| e.event_date).collect()
    }
}

impl TryFrom<Vec<CodedEvent>> for EventStream {
    type Error = QSError;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_preserves_category_and_event_date() {
        let error = Event::try_from(CodedEvent::new(Date::new(2027, 1, 1), "value = ?;".into()))
            .unwrap_err();
        assert!(matches!(error.root_cause(), QSError::ParsingErr(_)));
        assert!(error
            .to_string()
            .contains("parsing script event dated 2027-01-01"));
        assert!(error.to_string().contains("line 1, column 9"));
    }
}
