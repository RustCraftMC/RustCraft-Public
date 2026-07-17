use std::collections::BTreeMap;

use super::manifest::ModId;

pub type TaskId = u64;

#[derive(Clone, Debug)]
pub struct ScheduledTask {
    pub id: TaskId,
    pub owner: ModId,
    pub due_tick: u64,
    pub callback_name: String,
}

#[derive(Default)]
pub struct ScriptScheduler {
    next_id: TaskId,
    tasks: BTreeMap<TaskId, ScheduledTask>,
}

impl ScriptScheduler {
    pub fn schedule(
        &mut self,
        owner: ModId,
        due_tick: u64,
        callback_name: impl Into<String>,
    ) -> TaskId {
        self.next_id = self.next_id.saturating_add(1);
        self.tasks.insert(
            self.next_id,
            ScheduledTask {
                id: self.next_id,
                owner,
                due_tick,
                callback_name: callback_name.into(),
            },
        );
        self.next_id
    }

    pub fn take_due(&mut self, tick: u64) -> Vec<ScheduledTask> {
        let due: Vec<_> = self
            .tasks
            .iter()
            .filter_map(|(&id, task)| (task.due_tick <= tick).then_some(id))
            .collect();
        due.into_iter()
            .filter_map(|id| self.tasks.remove(&id))
            .collect()
    }

    pub fn cancel_owner(&mut self, owner: &ModId) {
        self.tasks.retain(|_, task| &task.owner != owner);
    }
}
