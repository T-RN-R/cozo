use crate::algebra::op::RelationalAlgebra;
use crate::algebra::parser::RaBox;
use crate::data::tuple_set::{
    merge_binding_maps, next_tset_indices, shift_merge_binding_map, BindingMap, TupleSet,
};
use crate::ddl::reify::TableInfo;
use anyhow::Result;
use std::collections::BTreeSet;

pub(crate) const NAME_CARTESIAN: &str = "Cartesian";

pub(crate) struct CartesianJoin<'a> {
    pub(crate) left: RaBox<'a>,
    pub(crate) right: RaBox<'a>,
}

impl<'b> RelationalAlgebra for CartesianJoin<'b> {
    fn name(&self) -> &str {
        NAME_CARTESIAN
    }

    fn bindings(&self) -> Result<BTreeSet<String>> {
        let mut ret = self.left.bindings()?;
        ret.extend(self.right.bindings()?);
        Ok(ret)
    }

    fn binding_map(&self) -> Result<BindingMap> {
        let mut left = self.left.binding_map()?;
        let right = self.right.binding_map()?;
        shift_merge_binding_map(&mut left, right);
        Ok(left)
    }

    fn iter<'a>(&'a self) -> Result<Box<dyn Iterator<Item = Result<TupleSet>> + 'a>> {
        let left = self.left.iter()?;
        let it = CartesianJoinIter {
            left,
            right: &self.right,
            left_cache: None,
            right_cache: None,
            started: false,
        };
        Ok(Box::new(it))
    }

    fn identity(&self) -> Option<TableInfo> {
        None
    }
}

pub(crate) struct CartesianJoinIter<'a> {
    left: Box<dyn Iterator<Item = Result<TupleSet>> + 'a>,
    right: &'a RaBox<'a>,
    left_cache: Option<TupleSet>,
    right_cache: Option<Box<dyn Iterator<Item = Result<TupleSet>> + 'a>>,
    started: bool,
}

impl<'a> Iterator for CartesianJoinIter<'a> {
    type Item = Result<TupleSet>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            match self.left.next() {
                None => return None,
                Some(Err(e)) => return Some(Err(e)),
                Some(Ok(t)) => {
                    self.left_cache = Some(t);
                }
            }

            match self.right.iter() {
                Ok(it) => self.right_cache = Some(it),
                Err(e) => return Some(Err(e)),
            }
            self.started = true;
        }

        loop {
            match &self.left_cache {
                None => return None,
                Some(left_tset) => {
                    match &mut self.right_cache {
                        None => return None,
                        Some(right_iter) => {
                            match right_iter.next() {
                                None => {
                                    // rewind
                                    match self.left.next() {
                                        None => return None,
                                        Some(Err(e)) => return Some(Err(e)),
                                        Some(Ok(left_tset)) => match self.right.iter() {
                                            Ok(iter) => {
                                                self.right_cache = Some(iter);
                                                self.left_cache = Some(left_tset);
                                                continue;
                                            }
                                            Err(e) => {
                                                return Some(Err(e));
                                            }
                                        },
                                    }
                                }
                                Some(Err(e)) => {
                                    return Some(Err(e));
                                }
                                Some(Ok(right_tset)) => {
                                    let mut left_tset = left_tset.clone();
                                    left_tset.merge(right_tset);
                                    return Some(Ok(left_tset));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
