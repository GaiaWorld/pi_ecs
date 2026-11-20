#![cfg(feature = "rc")]
use pi_ecs_command::command::*;

#[test]
fn registration_and_receiver_qualification_are_transactional() {
    let mut hub = CommandHub::new().unwrap();
    let epoch = WorldEpoch::initial();
    assert!(hub.sender::<u32>(epoch).is_err());
    hub.register::<u32>(CommandCapacity::Unbounded).unwrap();
    hub.register::<u32>(CommandCapacity::Unbounded).unwrap();
    assert!(hub
        .register::<u32>(CommandCapacity::Bounded(
            std::num::NonZeroUsize::new(1).unwrap()
        ))
        .is_err());
    let sender = hub.sender::<u32>(epoch).unwrap();
    let receiver = hub.receiver::<u32>(epoch).unwrap();
    assert!(hub.receiver::<u32>(epoch).is_err());
    drop(receiver);
    let receiver = hub.receiver::<u32>(epoch).unwrap();
    drop(receiver);
    drop(sender);
    hub.close();
    assert!(hub.register::<u64>(CommandCapacity::Unbounded).is_err());
    assert!(hub.sender::<u32>(epoch).is_err());
    assert!(hub.receiver::<u32>(epoch).is_err());
}

#[test]
fn send_returns_capacity_input_and_batches_have_fixed_bounds() {
    let mut hub = CommandHub::new().unwrap();
    hub.register::<String>(CommandCapacity::Bounded(
        std::num::NonZeroUsize::new(2).unwrap(),
    ))
    .unwrap();
    let sender = hub.sender::<String>(WorldEpoch::initial()).unwrap();
    let clone = sender.clone();
    assert!(sender.send("first".into()).is_ok());
    assert!(clone.send("second".into()).is_ok());
    let rejected = sender.send("returned".into()).err().unwrap();
    assert_eq!(rejected.command, "returned");
    let mut receiver = hub.receiver::<String>(WorldEpoch::initial()).unwrap();
    {
        let mut batch = receiver.read().unwrap();
        assert_eq!(batch.next().as_deref(), Some("first"));
        assert!(clone.send("later".into()).is_ok());
        assert_eq!(batch.next().as_deref(), Some("second"));
        assert_eq!(batch.next(), None);
    }
    drop(receiver);
    let mut receiver = hub.receiver::<String>(WorldEpoch::initial()).unwrap();
    assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), vec!["later"]);
    assert!(sender.send("preserved".into()).is_ok());
    {
        let _batch = receiver.read().unwrap();
    }
    assert_eq!(
        receiver.read().unwrap().next().as_deref(),
        Some("preserved")
    );
    hub.close();
    assert!(receiver.read().is_err());
    assert_eq!(
        sender.send("closed".into()).err().unwrap().command,
        "closed"
    );
}

#[test]
fn timers_use_explicit_ticks_and_share_capacity_without_blocking_ready() {
    let mut hub = CommandHub::new().unwrap();
    hub.register::<u32>(CommandCapacity::Bounded(
        std::num::NonZeroUsize::new(3).unwrap(),
    ))
    .unwrap();
    let sender = hub.sender::<u32>(WorldEpoch::initial()).unwrap();
    let mut receiver = hub.receiver::<u32>(WorldEpoch::initial()).unwrap();
    let key = sender.schedule_at(SimulationTick(5), 50).ok().unwrap();
    let canceled = sender.schedule_at(SimulationTick(6), 60).ok().unwrap();
    assert!(sender.send(1).is_ok());
    assert_eq!(
        sender
            .schedule_at(SimulationTick(10), 100)
            .err()
            .unwrap()
            .command,
        100
    );
    assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), vec![1]);
    assert!(sender.cancel(canceled).unwrap());
    assert!(!sender.cancel(canceled).unwrap());
    let immediate = sender.schedule_at(SimulationTick(0), 2).ok().unwrap();
    assert!(!sender.cancel(immediate).unwrap());
    hub.advance(SimulationTick(4)).unwrap();
    {
        let mut batch = receiver.read().unwrap();
        hub.advance(SimulationTick(5)).unwrap();
        assert_eq!(batch.next(), Some(2));
        assert_eq!(batch.next(), None);
    }
    assert!(!sender.cancel(key).unwrap());
    assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), vec![50]);
    assert!(hub.advance(SimulationTick(4)).is_err());
    hub.advance(SimulationTick(5)).unwrap();
    assert_eq!(receiver.read().unwrap().next(), None);
    let past = sender.schedule_at(SimulationTick(3), 3).ok().unwrap();
    assert!(!sender.cancel(past).unwrap());
    assert_eq!(receiver.read().unwrap().next(), Some(3));
}

#[test]
fn cancellation_keys_never_cross_services_types_or_reused_timer_slots() {
    let mut a = CommandHub::new().unwrap();
    let mut b = CommandHub::new().unwrap();
    for hub in [&mut a, &mut b] {
        hub.register::<u32>(CommandCapacity::Unbounded).unwrap();
    }
    a.register::<u64>(CommandCapacity::Unbounded).unwrap();
    let sa = a.sender::<u32>(WorldEpoch::initial()).unwrap();
    let sb = b.sender::<u32>(WorldEpoch::initial()).unwrap();
    let other = a.sender::<u64>(WorldEpoch::initial()).unwrap();
    let key = sa.schedule_at(SimulationTick(100), 1).ok().unwrap();
    assert!(!sb.cancel(key).unwrap());
    assert!(!other.cancel(key).unwrap());
    assert!(sa.cancel(key).unwrap());
    let new = sa.schedule_at(SimulationTick(100), 2).ok().unwrap();
    assert_ne!(key, new);
    assert!(!sa.cancel(key).unwrap());
    assert!(sa.cancel(new).unwrap());
    drop(a);
    assert!(sa.cancel(new).is_err());
    let mut c = CommandHub::new().unwrap();
    c.register::<u32>(CommandCapacity::Unbounded).unwrap();
    let sc = c.sender::<u32>(WorldEpoch::initial()).unwrap();
    assert_ne!(key, sc.schedule_at(SimulationTick(100), 3).ok().unwrap());
}

#[derive(Clone)]
struct Probe {
    id: u32,
    drops: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
    callback: Option<fn()>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.drops.lock().unwrap().push(self.id);
        if let Some(callback) = self.callback {
            callback();
        }
    }
}
thread_local! {
    static REENTER: std::cell::RefCell<Option<CommandSender<Probe>>> = const { std::cell::RefCell::new(None) };
}
fn reenter_open() {
    REENTER.with(|slot| {
        let slot = slot.borrow();
        let sender = slot.as_ref().unwrap();
        assert!(sender
            .send(Probe {
                id: 99,
                drops: Default::default(),
                callback: None
            })
            .is_ok());
    });
}
fn reenter_closed() {
    REENTER.with(|slot| {
        let slot = slot.borrow();
        let sender = slot.as_ref().unwrap();
        assert!(sender
            .send(Probe {
                id: 99,
                drops: Default::default(),
                callback: None
            })
            .is_err());
    });
}
#[test]
fn cancellation_and_close_release_payloads_once_outside_internal_borrows() {
    for explicit in [false, true] {
        let drops = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut hub = CommandHub::new().unwrap();
        hub.register::<Probe>(CommandCapacity::Bounded(
            std::num::NonZeroUsize::new(3).unwrap(),
        ))
        .unwrap();
        let sender = hub.sender::<Probe>(WorldEpoch::initial()).unwrap();
        REENTER.with(|slot| *slot.borrow_mut() = Some(sender.clone()));
        let key = sender
            .schedule_at(
                SimulationTick(20),
                Probe {
                    id: 1,
                    drops: drops.clone(),
                    callback: Some(reenter_open),
                },
            )
            .ok()
            .unwrap();
        assert!(sender.cancel(key).unwrap());
        assert_eq!(*drops.lock().unwrap(), vec![1]);
        let mut receiver = hub.receiver::<Probe>(WorldEpoch::initial()).unwrap();
        drop(receiver.read().unwrap().next().unwrap());
        assert!(sender
            .send(Probe {
                id: 2,
                drops: drops.clone(),
                callback: Some(reenter_closed)
            })
            .is_ok());
        assert!(sender
            .schedule_at(
                SimulationTick(30),
                Probe {
                    id: 3,
                    drops: drops.clone(),
                    callback: Some(reenter_closed)
                }
            )
            .is_ok());
        let mut batch = receiver.read().unwrap();
        if explicit {
            hub.close();
            hub.close();
        }
        drop(hub);
        assert!(batch.next().is_none());
        drop(batch);
        assert!(receiver.read().is_err());
        let mut actual = drops.lock().unwrap().clone();
        actual.sort();
        assert_eq!(actual, vec![1, 2, 3]);
        drop(receiver);
        REENTER.with(|slot| *slot.borrow_mut() = None);
        drop(sender);
        let mut actual = drops.lock().unwrap().clone();
        actual.sort();
        assert_eq!(actual, vec![1, 2, 3]);
    }
}

#[test]
fn due_order_matches_dependency_and_late_registration_uses_current_clock() {
    let mut hub = CommandHub::new().unwrap();
    hub.advance(SimulationTick(300)).unwrap();
    hub.register::<u32>(CommandCapacity::Unbounded).unwrap();
    let sender = hub.sender::<u32>(WorldEpoch::initial()).unwrap();
    let mut receiver = hub.receiver::<u32>(WorldEpoch::initial()).unwrap();
    let mut reference: pi_cancel_timer::Timer<u32, 256, 64, 3, 1> =
        pi_cancel_timer::Timer::new(300);
    for (tick, value) in [(301, 1), (560, 2), (560, 3), (561, 4)] {
        assert!(sender.schedule_at(SimulationTick(tick), value).is_ok());
        reference.schedule_at(tick, value);
    }
    for now in [300, 301, 559, 560, 561] {
        let mut expected = Vec::new();
        while let Some(value) = reference.pop(now) {
            expected.push(value);
        }
        hub.advance(SimulationTick(now)).unwrap();
        assert_eq!(receiver.read().unwrap().collect::<Vec<_>>(), expected);
    }
}

fn panic_on_drop() {
    panic!("user destructor");
}
#[test]
fn destructor_panic_keeps_service_closed_without_retrying_removed_values() {
    let drops = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut hub = CommandHub::new().unwrap();
    hub.register::<Probe>(CommandCapacity::Unbounded).unwrap();
    let sender = hub.sender::<Probe>(WorldEpoch::initial()).unwrap();
    let mut receiver = hub.receiver::<Probe>(WorldEpoch::initial()).unwrap();
    assert!(sender
        .send(Probe {
            id: 7,
            drops: drops.clone(),
            callback: Some(panic_on_drop)
        })
        .is_ok());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hub.close()));
    assert!(result.is_err());
    assert_eq!(*drops.lock().unwrap(), vec![7]);
    assert!(receiver.read().is_err());
    assert!(hub.advance(SimulationTick(1)).is_err());
    let rejected = sender
        .schedule_at(
            SimulationTick(1),
            Probe {
                id: 8,
                drops: drops.clone(),
                callback: None,
            },
        )
        .err()
        .unwrap();
    assert_eq!(rejected.command.id, 8);
    hub.close();
    drop(hub);
    assert_eq!(*drops.lock().unwrap(), vec![7]);
    drop(rejected);
    assert_eq!(*drops.lock().unwrap(), vec![7, 8]);
}

#[test]
fn empty_service_can_close_repeatedly() {
    let mut hub = CommandHub::new().expect("construct service");
    hub.close();
    hub.close();
}
