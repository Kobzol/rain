use std::collections::HashMap;
use common::wrapped::WrappedRcRefCell;
use common::id::{SessionId, WorkerId, DataObjectId, TaskId, ClientId};
use super::{WorkerRef, TaskRef, DataObjectRef, SessionRef, ClientRef};

#[derive(Clone, Default)]
pub struct Graph {
    /// Contained objects
    pub (in super::super) workers: HashMap<WorkerId, WorkerRef>,
    pub (in super::super) tasks: HashMap<TaskId, TaskRef>,
    pub (in super::super) objects: HashMap<DataObjectId, DataObjectRef>,
    pub (in super::super) sessions: HashMap<SessionId, SessionRef>,
    pub (in super::super) clients: HashMap<ClientId, ClientRef>,

    /// The last SessionId assigned.
    session_id_counter: SessionId,
}

impl Graph {

    pub fn new() -> Self {
        Default::default()
    }

    pub fn new_session_id(&mut self) -> SessionId {
        self.session_id_counter += 1;
        self.session_id_counter
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{ClientRef, SessionRef, Graph, TaskRef, WorkerRef, DataObjectRef};
    use common::id::{SId, TaskId, SessionId, ClientId, DataObjectId, WorkerId};
    use common::resources::Resources;

    fn create_test_graph(workers: i32, clients: i32, sessions: i32, tasks: i32, objects: i32) ->
                                                                                           Graph {
        let mut g = Graph::new();
        for wi in 0..workers {
            let w = WorkerRef::new(&mut g, format!("0.0.0.{}:67", wi + 1).parse().unwrap(), None,
                                   Resources { n_cpus: 8, });
        }
        for ci in 0..clients {
            let c = ClientRef::new(&mut g, format!("0.0.0.{}:42", ci + 1).parse().unwrap());
            for si in 0..sessions {
                let s = SessionRef::new(&mut g, &c);
                for ti in 0..tasks {
                    let t = TaskRef::new(&mut g, &s, TaskId::new(s.get_id(), ti));
                }
                for oi in 0..objects {
                    let o = DataObjectRef::new(&mut g, &s, DataObjectId::new(s.get_id(), oi +
                        tasks));
                }
            }
        }
        // TODO: add some links (objects, tasks, workers)
        g
    }

    #[test]
    fn graph_create_delete() {
        let mut g = create_test_graph(4, 2, 3, 10, 20);

        assert!(!g.objects.is_empty());
        assert!(!g.workers.is_empty());

        let client_rcs: Vec<_> = g.clients.values().map(|x| x.clone()).collect();
        let worker_rcs: Vec<_> = g.workers.values().map(|x| x.clone()).collect();
        for c in client_rcs { c.delete(&mut g); }
        for w in worker_rcs { w.delete(&mut g); }

        assert!(g.clients.is_empty());
        assert!(g.workers.is_empty());
        assert!(g.tasks.is_empty());
        assert!(g.objects.is_empty());
        assert!(g.sessions.is_empty());
    }
}