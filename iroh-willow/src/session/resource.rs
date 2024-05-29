use std::{
    collections::{HashMap, VecDeque},
    task::{Context, Poll, Waker},
};

use crate::proto::sync::{
    AreaOfInterestHandle, CapabilityHandle, IsHandle, ReadCapability, ResourceHandle,
    SetupBindAreaOfInterest, StaticToken, StaticTokenHandle,
};

use super::Error;

#[derive(Debug, Default)]
pub struct ResourceMaps {
    pub capabilities: ResourceMap<CapabilityHandle, ReadCapability>,
    pub areas_of_interest: ResourceMap<AreaOfInterestHandle, SetupBindAreaOfInterest>,
    pub static_tokens: ResourceMap<StaticTokenHandle, StaticToken>,
}
impl ResourceMaps {
    pub fn register_waker(&mut self, handle: ResourceHandle, waker: Waker) {
        tracing::trace!(?handle, "register_notify");
        match handle {
            ResourceHandle::AreaOfInterest(h) => self.areas_of_interest.register_waker(h, waker),
            ResourceHandle::Capability(h) => self.capabilities.register_waker(h, waker),
            ResourceHandle::StaticToken(h) => self.static_tokens.register_waker(h, waker),
            ResourceHandle::Intersection(_h) => unimplemented!(),
        }
    }

    pub fn get<F, H: IsHandle, R: Eq + PartialEq + Clone>(
        &self,
        selector: F,
        handle: H,
    ) -> Result<R, Error>
    where
        F: for<'a> Fn(&'a Self) -> &'a ResourceMap<H, R>,
    {
        let res = selector(&self);
        res.try_get(&handle).cloned()
    }

    pub fn poll_get_eventually<F, H: IsHandle, R: Eq + PartialEq + Clone>(
        &mut self,
        selector: F,
        handle: H,
        cx: &mut Context<'_>,
    ) -> Poll<R>
    where
        F: for<'a> Fn(&'a mut Self) -> &'a mut ResourceMap<H, R>,
    {
        let res = selector(self);
        let r = std::task::ready!(res.poll_get_eventually(handle, cx));
        Poll::Ready(r.clone())
    }
}

#[derive(Debug)]
pub struct ResourceMap<H, R> {
    next_handle: u64,
    map: HashMap<H, Resource<R>>,
    wakers: HashMap<H, VecDeque<Waker>>,
}

impl<H, R> Default for ResourceMap<H, R> {
    fn default() -> Self {
        Self {
            next_handle: 0,
            map: Default::default(),
            wakers: Default::default(),
        }
    }
}

impl<H, R> ResourceMap<H, R>
where
    H: IsHandle,
    R: Eq + PartialEq,
{
    pub fn iter(&self) -> impl Iterator<Item = (&H, &R)> + '_ {
        self.map.iter().map(|(h, r)| (h, &r.value))
    }

    pub fn bind(&mut self, resource: R) -> H {
        let handle: H = self.next_handle.into();
        self.next_handle += 1;
        let resource = Resource::new(resource);
        self.map.insert(handle, resource);
        tracing::trace!(?handle, "bind");
        if let Some(mut wakers) = self.wakers.remove(&handle) {
            tracing::trace!(?handle, "notify {}", wakers.len());
            for waker in wakers.drain(..) {
                waker.wake();
            }
        }
        handle
    }

    pub fn register_waker(&mut self, handle: H, notifier: Waker) {
        self.wakers.entry(handle).or_default().push_back(notifier)
    }

    pub fn bind_if_new(&mut self, resource: R) -> (H, bool) {
        // TODO: Optimize / find out if reverse index is better than find_map
        if let Some(handle) = self
            .map
            .iter()
            .find_map(|(handle, r)| (r.value == resource).then_some(handle))
        {
            (*handle, false)
        } else {
            let handle = self.bind(resource);
            (handle, true)
        }
    }

    pub fn try_get(&self, handle: &H) -> Result<&R, Error> {
        self.map
            .get(handle)
            .as_ref()
            .map(|r| &r.value)
            .ok_or_else(|| Error::MissingResource((*handle).into()))
    }

    pub fn get(&self, handle: &H) -> Option<&R> {
        self.map.get(handle).as_ref().map(|r| &r.value)
    }

    pub async fn get_eventually(&mut self, handle: H) -> &R {
        std::future::poll_fn(|ctx| {
            // cannot use self.get() and self.register_waker() here due to borrow checker.
            if let Some(resource) = self.map.get(&handle).as_ref().map(|r| &r.value) {
                Poll::Ready(resource)
            } else {
                self.wakers
                    .entry(handle)
                    .or_default()
                    .push_back(ctx.waker().to_owned());
                Poll::Pending
            }
        })
        .await
    }

    pub fn poll_get_eventually(&mut self, handle: H, cx: &mut Context<'_>) -> Poll<&R> {
        // cannot use self.get() and self.register_waker() here due to borrow checker.
        if let Some(resource) = self.map.get(&handle).as_ref().map(|r| &r.value) {
            Poll::Ready(resource)
        } else {
            self.wakers
                .entry(handle)
                .or_default()
                .push_back(cx.waker().to_owned());
            Poll::Pending
        }
    }
}

// #[derive(Debug)]
// enum ResourceState {
//     Active,
//     WeProposedFree,
//     ToBeDeleted,
// }

#[derive(Debug)]
struct Resource<V> {
    value: V,
    // state: ResourceState,
    // unprocessed_messages: usize,
}
impl<V> Resource<V> {
    pub fn new(value: V) -> Self {
        Self {
            value,
            // state: ResourceState::Active,
            // unprocessed_messages: 0,
        }
    }
}

// #[derive(Debug, Default)]
// pub struct Resources {
//     pub ours: ScopedResources,
//     pub theirs: ScopedResources,
// }
//
// impl Resources {
//     pub fn scope(&self, scope: Scope) -> &ScopedResources {
//         match scope {
//             Scope::Ours => &self.ours,
//             Scope::Theirs => &self.theirs,
//         }
//     }
//
//     pub fn scope_mut(&mut self, scope: Scope) -> &mut ScopedResources {
//         match scope {
//             Scope::Ours => &mut self.ours,
//             Scope::Theirs => &mut self.theirs,
//         }
//     }
// }
