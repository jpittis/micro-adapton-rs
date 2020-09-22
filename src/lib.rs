use slab::Slab;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Graph {
    athunks: Slab<RefCell<AThunk>>,
}

pub type Thunk = Box<dyn Fn(&mut Handle) -> f64>;

impl Graph {
    pub fn new() -> Self {
        Self {
            athunks: Slab::new(),
        }
    }

    pub fn new_athunk(&mut self, thunk: Thunk) -> AThunkID {
        let entry = self.athunks.vacant_entry();
        let id = AThunkID(entry.key());
        let athunk = RefCell::new(AThunk::new(id, thunk));
        entry.insert(athunk);
        id
    }

    pub fn new_aref(&mut self, val: f64) -> AThunkID {
        let thunk = Box::new(move |_: &mut Handle| val);
        self.new_athunk(thunk)
    }

    pub fn compute(&self, id: AThunkID, args: &[f64]) -> Option<f64> {
        Some(self.athunks.get(id.0)?.borrow_mut().compute(self, args))
    }

    pub fn update_aref(&mut self, id: AThunkID, val: f64) {
        {
            let mut aref = self.athunks.get(id.0).unwrap().borrow_mut();
            aref.thunk = Box::new(move |_: &mut Handle| val);
        }
        self.dirty(id);
    }

    fn dirty(&self, id: AThunkID) {
        let mut athunk = self.athunks.get(id.0).unwrap().borrow_mut();
        if athunk.clean {
            athunk.clean = false;
            athunk.result.clear();
            for &s in athunk.super_computations.iter() {
                self.dirty(s);
            }
        }
    }
}

pub struct Handle<'a, 'b, 'c> {
    pub args: &'c [f64],
    id: AThunkID,
    sub_computations: &'a mut HashSet<AThunkID>,
    graph: &'b Graph,
}

impl<'a, 'b, 'c> Handle<'a, 'b, 'c> {
    pub fn add_edge(&mut self, sub_id: AThunkID) {
        self.graph
            .athunks
            .get(sub_id.0)
            .unwrap()
            .borrow_mut()
            .super_computations
            .insert(self.id);
        self.sub_computations.insert(sub_id);
    }

    pub fn compute(&self, id: AThunkID, args: &[f64]) -> Option<f64> {
        self.graph.compute(id, args)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AThunkID(usize);

struct AThunk {
    id: AThunkID,
    thunk: Thunk,
    result: HashMap<Vec<u64>, f64>,
    clean: bool,
    sub_computations: HashSet<AThunkID>,
    super_computations: HashSet<AThunkID>,
}

impl AThunk {
    fn new(id: AThunkID, thunk: Thunk) -> Self {
        Self {
            id,
            thunk,
            result: HashMap::new(),
            sub_computations: HashSet::new(),
            super_computations: HashSet::new(),
            clean: false,
        }
    }

    fn compute(&mut self, g: &Graph, args: &[f64]) -> f64 {
        let key: Vec<u64> = args.iter().map(|&f| f as u64).collect();
        let result = self.result.get(&key);
        if self.clean {
            if let Some(&r) = result {
                return r;
            }
        }

        // Delete edge between self and sub_computations.
        for s in self.sub_computations.iter() {
            g.athunks
                .get(s.0)
                .unwrap()
                .borrow_mut()
                .super_computations
                .remove(&self.id);
        }
        self.sub_computations.clear();

        self.clean = true;
        let result = (self.thunk)(&mut Handle {
            args,
            id: self.id,
            sub_computations: &mut self.sub_computations,
            graph: g,
        });
        self.result.insert(key, result);
        self.compute(g, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut graph = Graph::new();

        let r1 = graph.new_aref(8.0);
        let r2 = graph.new_aref(10.0);
        let r3 = graph.new_aref(2.0);

        let a1 = graph.new_athunk(Box::new(move |h| {
            h.add_edge(r2);
            h.add_edge(r1);
            h.compute(r2, &[]).unwrap() - h.compute(r1, &[]).unwrap()
        }));

        let a2 = graph.new_athunk(Box::new(move |h| {
            h.add_edge(r3);
            h.add_edge(r1);
            h.compute(r3, &[]).unwrap() + h.compute(r1, &[]).unwrap()
        }));

        let a3 = graph.new_athunk(Box::new(move |h| {
            h.add_edge(r2);
            h.add_edge(a1);
            h.add_edge(a2);
            (h.compute(r2, &[]).unwrap()
                + h.compute(a1, &[]).unwrap()
                + h.compute(a2, &[]).unwrap())
                / h.args[0]
        }));

        assert_eq!(Some(22.0), graph.compute(a3, &[1.0]));
        assert_eq!(Some(11.0), graph.compute(a3, &[2.0]));
        assert_eq!(Some(22.0), graph.compute(a3, &[1.0]));
        assert_eq!(Some(11.0), graph.compute(a3, &[2.0]));

        graph.update_aref(r2, 6.0);

        assert_eq!(Some(14.0), graph.compute(a3, &[1.0]));
        assert_eq!(Some(7.0), graph.compute(a3, &[2.0]));
        assert_eq!(Some(14.0), graph.compute(a3, &[1.0]));
        assert_eq!(Some(7.0), graph.compute(a3, &[2.0]));
    }
}
