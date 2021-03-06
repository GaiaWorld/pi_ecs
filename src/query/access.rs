use crate::storage::FromOffset;
use fixedbitset::FixedBitSet;
use std::marker::PhantomData;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Access<T: FromOffset> {
    reads_all: bool,
    /// A combined set of T read and write accesses.
    reads_and_writes: FixedBitSet,
	reads: FixedBitSet,
    writes: FixedBitSet,
	modifys: FixedBitSet, // 访问可写，并且可发出事件

    marker: PhantomData<T>,
}

impl<T: FromOffset> Default for Access<T> {
    fn default() -> Self {
        Self {
            reads_all: false,
			reads: Default::default(),
            reads_and_writes: Default::default(),

            writes: Default::default(),
			modifys: Default::default(),
            marker: PhantomData,
        }
    }
}

impl<T: FromOffset> Access<T> {
    pub fn grow(&mut self, bits: usize) {
        self.reads_and_writes.grow(bits);
        self.writes.grow(bits);
		self.modifys.grow(bits);
    }

	pub fn get_reads_and_writes(&self) -> &FixedBitSet {
		&self.reads_and_writes
	}

	pub fn get_writes(&self) -> &FixedBitSet {
		&self.writes
	}

	pub fn get_modify(&self) -> &FixedBitSet {
		&self.modifys
	}

    pub fn add_read(&mut self, index: T) {
        self.reads_and_writes.grow(index.offset() + 1);
        self.reads_and_writes.insert(index.offset());
    }

    pub fn add_write(&mut self, index: T) {
        self.reads_and_writes.grow(index.offset() + 1);
        self.writes.grow(index.offset() + 1);
        self.reads_and_writes.insert(index.offset());
        self.writes.insert(index.offset());
    }

	pub fn add_modify(&mut self, index: T) {
		self.modifys.grow(index.offset() + 1);
		self.reads_and_writes.grow(index.offset() + 1);
		self.writes.grow(index.offset() + 1);

		self.modifys.insert(index.offset());
		self.reads_and_writes.insert(index.offset());
		self.writes.insert(index.offset());
	}

    pub fn has_read(&self, index: T) -> bool {
        self.reads_and_writes.contains(index.offset())
    }

	pub fn has_write(&self, index: T) -> bool {
        self.writes.contains(index.offset())
    }

    pub fn read_all(&mut self) {
        self.reads_all = true;
    }

    pub fn reads_all(&self) -> bool {
        self.reads_all
    }

    pub fn clear(&mut self) {
        self.reads_all = false;
        self.reads_and_writes.clear();
        self.writes.clear();
    }

    pub fn extend(&mut self, other: &Access<T>) {
        self.reads_all = self.reads_all || other.reads_all;
        self.reads_and_writes.union_with(&other.reads_and_writes);
        self.writes.union_with(&other.writes);
		self.modifys.union_with(&other.modifys);
    }

    pub fn is_compatible(&self, other: &Access<T>) -> bool {
        if self.reads_all {
            0 == other.writes.count_ones(..)
        } else if other.reads_all {
            0 == self.writes.count_ones(..)
        } else {
            self.writes.is_disjoint(&other.reads_and_writes)
                && self.reads_and_writes.is_disjoint(&other.writes)
        }
    }

    pub fn get_conflicts(&self, other: &Access<T>) -> Vec<T> {
        let mut conflicts = FixedBitSet::default();
        if self.reads_all {
            conflicts.extend(other.writes.ones());
        }

        if other.reads_all {
            conflicts.extend(self.writes.ones());
        }
        conflicts.extend(self.writes.intersection(&other.reads_and_writes));
        conflicts.extend(self.reads_and_writes.intersection(&other.writes));
        conflicts
            .ones()
            .map(FromOffset::from_offset)
            .collect()
    }
}

#[derive(Clone)]
pub struct FilteredAccess<T: FromOffset> {
    access: Access<T>,
    with: FixedBitSet,
    without: FixedBitSet,
}

impl<T: FromOffset> Default for FilteredAccess<T> {
    fn default() -> Self {
        Self {
            access: Access::default(),
            with: Default::default(),
            without: Default::default(),
        }
    }
}

impl<T: FromOffset> FilteredAccess<T> {
    #[inline]
    pub fn access(&self) -> &Access<T> {
        &self.access
    }

    pub fn add_read(&mut self, index: T) {
        self.access.add_read(index.clone());
        self.add_with(index);
    }

    pub fn add_write(&mut self, index: T) {
        self.access.add_write(index.clone());
        self.add_with(index);
    }

	pub fn add_modify(&mut self, index: T) {
        self.access.add_modify(index.clone());
        self.add_with(index);
    }

    pub fn add_with(&mut self, index: T) {
        self.with.grow(index.offset() + 1);
        self.with.insert(index.offset());
    }

    pub fn add_without(&mut self, index: T) {
        self.without.grow(index.offset() + 1);
        self.without.insert(index.offset());
    }

	pub fn has_read(&self, index: T) -> bool {
        self.access.has_read(index.clone())
    }

    pub fn has_write(&self, index: T) -> bool {
        self.access.has_write(index.clone())
    }

    pub fn is_compatible(&self, other: &FilteredAccess<T>) -> bool {
        if self.access.is_compatible(&other.access) {
            true
        } else {
            self.with.intersection(&other.without).next().is_some()
                || self.without.intersection(&other.with).next().is_some()
        }
    }
}

#[derive(Clone)]
pub struct FilteredAccessSet<T: FromOffset> {
    combined_access: Access<T>,
    filtered_accesses: Vec<FilteredAccess<T>>,
}

impl<T: FromOffset> FilteredAccessSet<T> {
    #[inline]
    pub fn combined_access(&self) -> &Access<T> {
        &self.combined_access
    }

    #[inline]
    pub fn combined_access_mut(&mut self) -> &mut Access<T> {
        &mut self.combined_access
    }

    pub fn get_conflicts(&self, filtered_access: &FilteredAccess<T>) -> Vec<T> {
        // if combined unfiltered access is incompatible, check each filtered access for
        // compatibility
        if !filtered_access.access.is_compatible(&self.combined_access) {
            for current_filtered_access in self.filtered_accesses.iter() {
                if !current_filtered_access.is_compatible(&filtered_access) {
                    return current_filtered_access
                        .access
                        .get_conflicts(&filtered_access.access);
                }
            }
        }
        Vec::new()
    }

	pub fn get_entity_conflicts(&self, filtered_access: &FilteredAccess<T>) -> Vec<T> {
        // if combined unfiltered access is incompatible, check each filtered access for
        // compatibility
        if !filtered_access.access.is_compatible(&self.combined_access) {
            for current_filtered_access in self.filtered_accesses.iter() {
                if !current_filtered_access.is_compatible(&filtered_access) {
                    return current_filtered_access
                        .access
                        .get_conflicts(&filtered_access.access);
                }
            }
        }
        Vec::new()
    }

    pub fn add(&mut self, filtered_access: FilteredAccess<T>) {
        self.combined_access.extend(&filtered_access.access);
        self.filtered_accesses.push(filtered_access);
    }
}

impl<T: FromOffset> Default for FilteredAccessSet<T> {
    fn default() -> Self {
        Self {
            combined_access: Default::default(),
            filtered_accesses: Vec::new(),
        }
    }
}

