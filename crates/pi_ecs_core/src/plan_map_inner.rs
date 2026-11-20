//! Alter 单数组映射配对芯；头部偏移相对于固定头部末尾。
use crate::{error::EcsError, plan_map::ArchetypePlanMap};
impl ArchetypePlanMap {
    pub(crate) fn new_inner(table_count: usize) -> Self {
        Self {
            entries: vec![0; table_count * 2],
            table_count,
        }
    }
    pub(crate) fn get_inner(_map: &Self, _table: u16, _variant: u16) -> Option<u16> {
        let table = usize::from(_table);
        if table >= _map.table_count {
            return None;
        }
        let count = usize::from(_map.entries[table * 2 + 1]);
        if usize::from(_variant) >= count {
            return None;
        }
        let offset = usize::from(_map.entries[table * 2]);
        let plan = *_map
            .entries
            .get(_map.table_count * 2 + offset + usize::from(_variant))?;
        (plan != u16::MAX).then_some(plan)
    }
    pub(crate) fn set_inner(
        _map: &mut Self,
        _table: u16,
        _variant: u16,
        _plan: Option<u16>,
    ) -> Result<(), EcsError> {
        let invalid = || {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::InvalidInput,
                "计划编号、Table 或变体序号无效",
            )
        };
        let exhausted = || {
            pi_ecs_types::error::classified_error(
                pi_ecs_types::error::ErrorKind::ResourceExhausted,
                "计划映射压缩容量耗尽",
            )
        };
        let table = usize::from(_table);
        let variant = usize::from(_variant);
        if table >= _map.table_count || _plan == Some(u16::MAX) {
            return Err(invalid());
        }
        let count = usize::from(_map.entries[table * 2 + 1]);
        if variant > count {
            return Err(invalid());
        }
        let value = _plan.unwrap_or(u16::MAX);
        let head = _map.table_count * 2;
        if variant < count {
            let offset = usize::from(_map.entries[table * 2]);
            _map.entries[head + offset + variant] = value;
            return Ok(());
        }
        // 临时压缩所有有效段；容量拒绝和分配失败之前不发布任何头部或尾部。
        let next_count = count.checked_add(1).ok_or_else(exhausted)?;
        u16::try_from(next_count).map_err(|_| exhausted())?;
        let mut entries = vec![0; head];
        for index in 0.._map.table_count {
            let old_count = usize::from(_map.entries[index * 2 + 1]);
            let new_count = old_count + usize::from(index == table);
            if new_count == 0 {
                continue;
            }
            let offset = u16::try_from(entries.len() - head).map_err(|_| exhausted())?;
            let encoded_count = u16::try_from(new_count).map_err(|_| exhausted())?;
            entries[index * 2] = offset;
            entries[index * 2 + 1] = encoded_count;
            let old_offset = usize::from(_map.entries[index * 2]);
            entries
                .extend_from_slice(&_map.entries[head + old_offset..head + old_offset + old_count]);
            if index == table {
                entries.push(value);
            }
        }
        _map.entries = entries;
        Ok(())
    }
    pub(crate) fn clear_inner(map: &mut Self) {
        map.entries.clear();
        map.entries.resize(map.table_count * 2, 0);
    }
}
// 通过现有映射接口验收哨兵、容量失败原子性和重建。
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_get_and_rebuild() {
        let mut map = ArchetypePlanMap::new(2);
        assert_eq!(map.get(0, 0), None);
        map.set(0, 0, Some(0)).unwrap();
        map.set(0, 1, Some(1)).unwrap();
        map.set(1, 0, Some(2)).unwrap();
        assert_eq!(map.get(0, 0), Some(0));
        assert_eq!(map.get(0, 1), Some(1));
        assert_eq!(map.get(1, 0), Some(2));
        map.set(0, 0, Some(3)).unwrap();
        assert_eq!(map.get(0, 0), Some(3));
        map.clear();
        assert_eq!(map.get(0, 0), None);
    }
    #[test]
    fn rejects_capacity_and_variant_gap() {
        let mut map = ArchetypePlanMap::new(1);
        assert!(map.set(0, 0, Some(u16::MAX)).is_err());
        assert!(map.set(0, 1, Some(0)).is_err());
        assert!(map.set(1, 0, Some(0)).is_err());
    }
    #[test]
    fn compression_capacity_failure_preserves_every_mapping() {
        let mut map = ArchetypePlanMap::new(3);
        for variant in 0..40000 {
            map.set(0, variant, Some(7)).unwrap();
        }
        for variant in 0..30000 {
            map.set(1, variant, Some(11)).unwrap();
        }
        assert!(map.set(2, 0, Some(19)).is_err());
        assert_eq!(map.get(0, 39999), Some(7));
        assert_eq!(map.get(1, 29999), Some(11));
        assert_eq!(map.get(2, 0), None);
        // 覆盖不受尾部总长影响；失败追加未占据变体序号。
        map.set(1, 29999, None).unwrap();
        assert_eq!(map.get(1, 29999), None);
        map.clear();
        map.set(2, 0, Some(19)).unwrap();
        assert_eq!(map.get(2, 0), Some(19));
    }
    #[test]
    fn invalid_source_before_valid_source() {
        let mut map = ArchetypePlanMap::new(1);
        map.set(0, 0, None).unwrap();
        map.set(0, 1, Some(0)).unwrap();
        assert_eq!(map.get(0, 0), None);
        assert_eq!(map.get(0, 1), Some(0));
        map.set(0, 1, None).unwrap();
        assert_eq!(map.get(0, 1), None);
    }
}
