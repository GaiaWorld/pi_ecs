use std::convert::From;
use std::ops::{Deref, DerefMut};
use std::{ops::Index, ops::IndexMut};

pub use pi_slotmap::{Key, KeyData, SlotMap, SecondaryMap as SecondaryMap1, SparseSecondaryMap as SparseSecondaryMap1, DenseSlotMap, DelaySlotMap};
pub use pi_map::Map;
pub use pi_slotmap::dense::{Iter, IterMut, Keys, Values};


pub struct SecondaryMap<K: Key, V>(SecondaryMap1<K, V>);

impl<K: Key, V> Deref for SecondaryMap<K, V> {
	type Target = SecondaryMap1<K, V>;
    fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<K: Key, V> DerefMut for SecondaryMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<K: Key, V> Map for SecondaryMap<K, V> {
	type Key = K;
	type Val = V;

	fn len(&self) -> usize {
		self.0.len()
	}
	fn with_capacity(capacity: usize) -> Self {
		SecondaryMap(SecondaryMap1::with_capacity(capacity))
	}
    fn capacity(&self) -> usize {
		self.0.len()
	}
    fn mem_size(&self) -> usize {
		self.0.len()
	}
    fn contains(&self, key: &Self::Key) -> bool {
		self.0.contains_key(*key)
	}
    fn get(&self, key: &Self::Key) -> Option<&Self::Val> {
		self.0.get(*key)
	}
    fn get_mut(&mut self, key: &Self::Key) -> Option<&mut Self::Val> {
		self.0.get_mut(*key)
	}
    unsafe fn get_unchecked(&self, key: &Self::Key) -> &Self::Val {
		self.0.get_unchecked(*key)
	}
    unsafe fn get_unchecked_mut(&mut self, key: &Self::Key) -> &mut Self::Val {
		self.0.get_unchecked_mut(*key)
	}
    unsafe fn remove_unchecked(&mut self, key: &Self::Key) -> Self::Val {
		self.0.remove(*key).unwrap()
	}
    fn insert(&mut self, key: Self::Key, val: Self::Val) -> Option<Self::Val> {
		self.0.insert(key, val)
	}
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Val> {
		self.0.remove(*key)
	}
}

impl<K: Key, V> Index<K> for SecondaryMap<K, V> {
	type Output = V;
    fn index(&self, index: K) -> &Self::Output {
		&self.0[index]
	}
}

impl<K: Key, V> IndexMut<K> for SecondaryMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
		&mut self.0[index]
	}
}

pub struct SparseSecondaryMap<K: Key, V>(SparseSecondaryMap1<K, V>);

impl<K: Key, V> Deref for SparseSecondaryMap<K, V> {
	type Target = SparseSecondaryMap1<K, V>;
    fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<K: Key, V> DerefMut for SparseSecondaryMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<K: Key, V> Map for SparseSecondaryMap<K, V> {
	type Key = K;
	type Val = V;

	fn len(&self) -> usize {
		self.0.len()
	}
	fn with_capacity(capacity: usize) -> Self {
		SparseSecondaryMap(SparseSecondaryMap1::with_capacity(capacity))
	}
    fn capacity(&self) -> usize {
		self.0.len()
	}
    fn mem_size(&self) -> usize {
		self.0.len()
	}
    fn contains(&self, key: &Self::Key) -> bool {
		self.0.contains_key(*key)
	}
    fn get(&self, key: &Self::Key) -> Option<&Self::Val> {
		self.0.get(*key)
	}
    fn get_mut(&mut self, key: &Self::Key) -> Option<&mut Self::Val> {
		self.0.get_mut(*key)
	}
    unsafe fn get_unchecked(&self, key: &Self::Key) -> &Self::Val {
		self.0.get_unchecked(*key)
	}
    unsafe fn get_unchecked_mut(&mut self, key: &Self::Key) -> &mut Self::Val {
		self.0.get_unchecked_mut(*key)
	}
    unsafe fn remove_unchecked(&mut self, key: &Self::Key) -> Self::Val {
		self.0.remove(*key).unwrap()
	}
    fn insert(&mut self, key: Self::Key, val: Self::Val) -> Option<Self::Val> {
		self.0.insert(key, val)
	}
    fn remove(&mut self, key: &Self::Key) -> Option<Self::Val> {
		self.0.remove(*key)
	}
}

impl<K: Key, V> Index<K> for SparseSecondaryMap<K, V> {
	type Output = V;
    fn index(&self, index: K) -> &Self::Output {
		&self.0[index]
	}
}

impl<K: Key, V> IndexMut<K> for SparseSecondaryMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
		&mut self.0[index]
	}
}

pub trait Offset: Clone {
	fn offset(&self) -> usize;
}
pub trait FromOffset: Offset {
	fn from_offset(offset: usize) -> Self;
}


#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct LocalVersion(pub(crate) u64);

// impl Null for LocalVersion {
// 	/// 判断当前值是否空
// 	fn null() -> Self {
// 		LocalVersion::new()
// 	}
// 	/// 判断当前值是否空
// 	fn is_null(&self) -> bool;
// }



// impl LocalVersion {
// 	pub(crate) fn new(idx: u32, version: u32) -> Self {
// 		LocalVersion((version as u64) << 32 +  idx as u64)
// 	}

// 	pub(crate) fn version(&self) -> u32 {
// 		(self.0 >> 32) as u32
// 	}
// }

impl Deref for LocalVersion {
	type Target = u64;

    fn deref(&self) -> &Self::Target {
		&self.0
	}
}
 
unsafe impl Key for LocalVersion {
	#[inline]
    fn data(&self) -> KeyData {
		KeyData::from_ffi(self.0)
	}
}

impl From<KeyData> for LocalVersion {
	#[inline]
    fn from(data: KeyData) -> Self {
		LocalVersion(data.as_ffi())
	}
}

impl Offset for LocalVersion {
	#[inline]
    fn offset(&self) -> usize {
		(self.0 & 0xffff_ffff) as usize
		// (self.0 << 32 >> 32) as usize
	}
}


#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct Local(usize);

impl Deref for Local {
	type Target = usize;

    fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl Local {
	#[inline]
	pub fn new(v: usize) -> Self {
		Self(v)
	}	
}

unsafe impl Key for Local {
	#[inline]
    fn data(&self) -> KeyData {
		KeyData::from_ffi(self.0 as u64 | 1 << 32)
	}
}

impl Offset for Local{
	#[inline]
    fn offset(&self) -> usize {
		self.0
	}
}

impl FromOffset for Local{
	#[inline]
    fn from_offset(offset: usize) -> Self {
		Local(offset)
	}
}

impl From<KeyData> for Local {
	#[inline]
    fn from(data: KeyData) -> Self {
		Local(data.as_ffi() as usize)
	}
}

