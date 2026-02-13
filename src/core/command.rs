use crate::core::contextmanager::ContextManager;

/// Command
pub trait Command {
    /// execute command
    fn execute(&mut self, ctx: &ContextManager) -> bool;
}

/// executor
pub fn execute_command(ctx: &ContextManager, command: impl Command) {

}