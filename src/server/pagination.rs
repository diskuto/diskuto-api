use std::marker::PhantomData;

use serde::Deserialize;

use crate::backend::{TimeSpan, Timestamp};

/// Query params to control pagination:
#[derive(Deserialize, Debug)]
pub(crate) struct Pagination {
    /// Time before which to show posts. Default is now.
    before: Option<i64>,

    /// Time after which to show some posts. can not set before & after, and before takes precedence.
    /// Note: posts will still be listed in reverse-chronological-order. (newest first).
    after: Option<i64>,

    // TODO: add sig (signature) here for correct pagination.

    /// Limit how many posts/items appear on a page.
    count: Option<usize>,
}


/// Works with the callbacks in Backend to provide pagination.
/// Handles max # items, tracking whether the source has_more items, 
/// and some rudamentary pagination link generation.
// This feels ... over-engineered? But OTOH I really don't want to have to write pagination logic multiple times?
// I'd be happy to hear about better alternatives here, especially if it's a crate. :) 
#[derive(Debug)]
pub(crate) struct Paginator<T, In, E, Mapper, Filter>
where 
    Mapper: Fn(In) -> Result<T,E>,
    Filter: Fn(&T) -> bool,
 {
    items: Vec<T>,
    pub has_more: bool,
    pub params: Pagination,
    pub max_items: usize,

    mapper: Mapper,
    filter: Filter,
    have_flipped: bool,

    _in: PhantomData<In>,
    _err: PhantomData<E>,
}

impl<T, In, E, Mapper, Filter> Paginator<T, In, E, Mapper, Filter>
where 
    Mapper: Fn(In) -> Result<T,E>,
    Filter: Fn(&T) -> bool,
{
    fn accept(&mut self, input: In) -> Result<bool, E> {
        let max_len = self.params.count.map(|c| bound(c, 1, self.max_items)).unwrap_or(self.max_items);
        
        let item = (self.mapper)(input)?;
        if !(self.filter)(&item) {
            return Ok(true); // continue
        }

        if self.items.len() >= max_len {
            self.has_more = true;
            return Ok(false); // stop
        }

        self.items.push(item);
        return Ok(true)
    }

    pub fn callback<'a>(&'a mut self) -> impl FnMut(In) -> Result<bool, E> + 'a {
        move |input| self.accept(input)
    }

    /// Creates a new paginator for collecting results from a Backend.
    /// mapper: Maps the row type passed to the callback to some other type.
    /// filter: Filters that type for inclusion in the paginated results.
    pub fn new(params: Pagination, mapper: Mapper, filter: Filter) -> Self {
        Self {
            params,
            items: vec![],
            // Seems like a reasonable sane default for things that have to hold Item in memory:
            max_items: 100,
            has_more: false,
            mapper,
            filter,
            have_flipped: false,
            _in: PhantomData,
            _err: PhantomData,
        }
    }

    /// The time before which we should query for items.
    /// Prefer time_span() if bidirectional pagination is supported.
    /// TODO: Deprecate pagination by "before" only:
    pub fn before(&self) -> Timestamp {
        self.params.before.map(|t| Timestamp{ unix_utc_ms: t}).unwrap_or_else(|| Timestamp::now())
    }

    /// The time span we should display for the current request:
    pub fn time_span(&self) -> TimeSpan {
        // If both are specified, prefer "before":
        if let Some(before) = self.params.before {
            return TimeSpan::Before(Timestamp { unix_utc_ms: before });
        }
        if let Some(after) = self.params.after {
            return TimeSpan::After(Timestamp { unix_utc_ms: after });
        }

        // else:
        TimeSpan::Before(Timestamp::now())
    }

    fn flip_items(&mut self) {
        if !self.time_span().is_before() && !self.have_flipped {
            // Then we were iterating in backwards order, and need to flip
            self.items.reverse();
            self.have_flipped = true;
        }
    }

    pub fn into_items(mut self) -> Vec<T> {
        self.flip_items();
        let Self{items, ..} = self;
        items
    }
}

/// Set lower and upper bounds for input T.
fn bound<T: Ord>(input: T, lower: T, upper: T) -> T {
    use std::cmp::{min, max};
    min(max(lower, input), upper)
}

