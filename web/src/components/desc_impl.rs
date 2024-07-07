use crate::components::context::ExpandedDescriptions;
use std::rc::Rc;
use yewdux::prelude::*;

#[allow(dead_code)]
pub enum AppStateMsg {
    ExpandEpisode(String),
    CollapseEpisode(String),
}

impl Reducer<ExpandedDescriptions> for AppStateMsg {
    fn apply(self, mut state: Rc<ExpandedDescriptions>) -> Rc<ExpandedDescriptions> {
        let state_mut = Rc::make_mut(&mut state);

        match self {
            AppStateMsg::ExpandEpisode(guid) => {
                state_mut.expanded_descriptions.insert(guid);
            }
            AppStateMsg::CollapseEpisode(guid) => {
                state_mut.expanded_descriptions.remove(&guid);
            }
        }

        // Return the Rc itself, not a reference to it
        state
    }
}
