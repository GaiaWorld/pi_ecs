//! 命令服务配对芯；RC 类型化队列与短借生命周期。
#[cfg(feature = "rc")]
use pi_ecs_types::error::{classified_error, ErrorKind};
#[cfg(feature = "rc")]
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
};

#[cfg(feature = "rc")]
impl CommandHub {
    pub(super) fn new_inner() -> Result<Self, EcsError> {
        // 全进程分配，跨 RC 线程也不复用服务身份；最大值保留为耗尽哨兵。
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
                |id| id.checked_add(1),
            )
            .map_err(|_| classified_error(ErrorKind::ResourceExhausted, "命令服务身份耗尽"))?;
        Ok(Self {
            endpoint: CommandEndpoint {
                control: Share::new(CommandControl {
                    identity,
                    epoch: WorldEpoch::initial(),
                    closed: Cell::new(false),
                    now: Cell::new(SimulationTick(0)),
                }),
                queues: HashMap::new(),
                next_queue: 1,
            },
        })
    }
    pub(super) fn close_inner(&mut self) {
        self.endpoint.control.closed.set(true);
        // 移走目录后，即使用户析构 panic，剩余目录也按普通所有权释放。
        let queues = std::mem::take(&mut self.endpoint.queues);
        for entry in queues.values() {
            (entry.close)(entry.typed.as_ref());
        }
    }
    pub(super) fn drop_inner(&mut self) {
        self.close_inner();
    }
    pub(super) fn register_inner<C: Send + 'static>(
        &mut self,
        capacity: CommandCapacity,
    ) -> Result<(), EcsError> {
        validate(&self.endpoint.control, self.endpoint.control.epoch)?;
        if let Some(entry) = self.endpoint.queues.get(&TypeId::of::<C>()) {
            let queue = typed_queue::<C>(entry.typed.as_ref());
            return if queue.capacity == capacity {
                Ok(())
            } else {
                Err(classified_error(ErrorKind::InvalidInput, "队列容量不一致"))
            };
        }
        let next = self
            .endpoint
            .next_queue
            .checked_add(1)
            .ok_or_else(|| classified_error(ErrorKind::ResourceExhausted, "队列身份耗尽"))?;
        let queue = Share::new(CommandQueue::<C> {
            control: self.endpoint.control.clone(),
            identity: self.endpoint.next_queue,
            capacity,
            state: RefCell::new(CommandQueueState {
                pending: HashMap::new(),
                ready: VecDeque::new(),
                timer: pi_cancel_timer::Timer::new(self.endpoint.control.now.get().0),
                next_command: 1,
                next_ready: 0,
                occupied: false,
                last_receiver_claim: 0,
            }),
        });
        self.endpoint.queues.insert(
            TypeId::of::<C>(),
            QueueEntry {
                receiver_claimed: |typed| typed_queue::<C>(typed).state.borrow().occupied,
                typed: Box::new(queue),
                advance: advance_queue::<C>,
                close: close_queue::<C>,
            },
        );
        self.endpoint.next_queue = next;
        Ok(())
    }
    fn queue<C: Send + 'static>(
        &self,
        epoch: WorldEpoch,
    ) -> Result<Share<CommandQueue<C>>, EcsError> {
        validate(&self.endpoint.control, epoch)?;
        let entry = self
            .endpoint
            .queues
            .get(&TypeId::of::<C>())
            .ok_or_else(|| classified_error(ErrorKind::NotFound, "命令类型未登记"))?;
        Ok(typed_queue::<C>(entry.typed.as_ref()).clone())
    }
    pub(super) fn sender_inner<C: Send + 'static>(
        &self,
        epoch: WorldEpoch,
    ) -> Result<CommandSender<C>, EcsError> {
        Ok(CommandSender {
            queue: self.queue::<C>(epoch)?,
            epoch,
            marker: PhantomData,
        })
    }
    pub(super) fn receiver_inner<C: Send + 'static>(
        &mut self,
        epoch: WorldEpoch,
    ) -> Result<CommandReceiver<C>, EcsError> {
        let queue = self.queue::<C>(epoch)?;
        let claim = {
            let mut state = queue.state.borrow_mut();
            if state.occupied {
                return Err(classified_error(ErrorKind::AlreadyExists, "消费资格已领取"));
            }
            let claim = state.last_receiver_claim.checked_add(1).ok_or_else(|| {
                classified_error(ErrorKind::ResourceExhausted, "命令消费资格编号耗尽")
            })?;
            state.last_receiver_claim = claim;
            state.occupied = true;
            claim
        };
        Ok(CommandReceiver {
            claim,
            queue,
            epoch,
            marker: PhantomData,
        })
    }
    pub(super) fn advance_inner(&mut self, now: SimulationTick) -> Result<(), EcsError> {
        let control = &self.endpoint.control;
        validate(control, control.epoch)?;
        if now < control.now.get() {
            return Err(classified_error(
                ErrorKind::InvalidInput,
                "模拟时间不能倒退",
            ));
        }
        // 在跨队列推进期间保持撤销态；内部 panic 无法重新开放服务。
        control.closed.set(true);
        for entry in self.endpoint.queues.values() {
            (entry.advance)(entry.typed.as_ref(), now);
        }
        control.now.set(now);
        control.closed.set(false);
        Ok(())
    }
}
#[cfg(feature = "rc")]
impl<C: Send + 'static> CommandSender<C> {
    pub(super) fn send_inner(&self, command: C) -> Result<(), CommandSendError<C>> {
        if let Err(error) = validate(&self.queue.control, self.epoch) {
            return Err(CommandSendError { error, command });
        }
        let mut state = self.queue.state.borrow_mut();
        if let Err(error) = prepare_receive(&self.queue, &mut state) {
            drop(state);
            return Err(CommandSendError { error, command });
        }
        let id = state.next_command;
        state.next_command += 1;
        state.pending.insert(
            id,
            CommandEntry {
                value: command,
                timer_key: None,
            },
        );
        publish_ready(&mut state, id);
        Ok(())
    }
    pub(super) fn schedule_inner(
        &self,
        deadline: SimulationTick,
        command: C,
    ) -> Result<TimerKey, CommandSendError<C>> {
        if let Err(error) = validate(&self.queue.control, self.epoch) {
            return Err(CommandSendError { error, command });
        }
        let mut state = self.queue.state.borrow_mut();
        if let Err(error) = prepare_receive(&self.queue, &mut state) {
            drop(state);
            return Err(CommandSendError { error, command });
        }
        let id = state.next_command;
        let timer_key = if deadline > self.queue.control.now.get() {
            Some(state.timer.schedule_at(deadline.0, id))
        } else {
            None
        };
        state.next_command += 1;
        state.pending.insert(
            id,
            CommandEntry {
                value: command,
                timer_key,
            },
        );
        if timer_key.is_none() {
            publish_ready(&mut state, id);
        }
        Ok(TimerKey {
            owner: owner(&self.queue),
            slot: id,
        })
    }
    pub(super) fn cancel_inner(&self, key: TimerKey) -> Result<bool, EcsError> {
        validate(&self.queue.control, self.epoch)?;
        if key.owner != owner(&self.queue) {
            return Ok(false);
        }
        let removed = {
            let mut state = self.queue.state.borrow_mut();
            let Some(timer_key) = state
                .pending
                .get(&key.slot)
                .and_then(|entry| entry.timer_key)
            else {
                return Ok(false);
            };
            let id = state.timer.cancel(timer_key).expect("延迟索引存在");
            assert_eq!(id, key.slot, "取消键对应唯一命令");
            state.pending.remove(&id)
        };
        // 释放名额和 RefCell 短借后，用户 Drop 才可重入服务。
        drop(removed);
        Ok(true)
    }
    pub(super) fn clone_inner(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            epoch: self.epoch,
            marker: PhantomData,
        }
    }
}
#[cfg(feature = "rc")]
impl<C: Send + 'static> CommandReceiver<C> {
    pub(super) fn read_inner(&mut self) -> Result<CommandBatch<'_, C>, EcsError> {
        validate(&self.queue.control, self.epoch)?;
        let end = self.queue.state.borrow().next_ready;
        Ok(CommandBatch {
            receiver: self,
            end,
        })
    }
    pub(super) fn receiver_drop_inner(&mut self) {
        self.queue.state.borrow_mut().occupied = false;
    }
}
#[cfg(feature = "rc")]
impl<C: Send + 'static> CommandBatch<'_, C> {
    pub(super) fn next_inner(&mut self) -> Option<C> {
        if validate(&self.receiver.queue.control, self.receiver.epoch).is_err() {
            return None;
        }
        let mut state = self.receiver.queue.state.borrow_mut();
        if state.ready.front()?.0 >= self.end {
            return None;
        }
        let (_, id) = state.ready.pop_front().expect("就绪头存在");
        let entry = state.pending.remove(&id).expect("就绪命令唯一拥有");
        drop(state);
        Some(entry.value)
    }
}

#[cfg(feature = "rc")]
fn validate(control: &CommandControl, epoch: WorldEpoch) -> Result<(), EcsError> {
    if control.closed.get() {
        return Err(classified_error(ErrorKind::InvalidState, "命令服务已关闭"));
    }
    if control.epoch != epoch {
        return Err(classified_error(ErrorKind::InvalidState, "旧命令代次"));
    }
    Ok(())
}
#[cfg(feature = "rc")]
fn typed_queue<C: Send + 'static>(typed: &dyn Any) -> &Share<CommandQueue<C>> {
    typed
        .downcast_ref::<Share<CommandQueue<C>>>()
        .expect("命令目录类型一致")
}
#[cfg(feature = "rc")]
fn close_queue<C: Send + 'static>(typed: &dyn Any) {
    let queue = typed_queue::<C>(typed);
    let pending = {
        let mut state = queue.state.borrow_mut();
        state.ready.clear();
        // timer 仅含索引；清空后再在短借外释放唯一 payload。
        state.timer = pi_cancel_timer::Timer::new(queue.control.now.get().0);
        std::mem::take(&mut state.pending)
    };
    drop(pending);
}
#[cfg(feature = "rc")]
fn advance_queue<C: Send + 'static>(typed: &dyn Any, now: SimulationTick) {
    let queue = typed_queue::<C>(typed);
    let mut state = queue.state.borrow_mut();
    while let Some(id) = state.timer.pop(now.0) {
        state
            .pending
            .get_mut(&id)
            .expect("延迟命令唯一拥有")
            .timer_key = None;
        publish_ready(&mut state, id);
    }
}
#[cfg(feature = "rc")]
fn owner<C>(queue: &CommandQueue<C>) -> u128 {
    (u128::from(queue.control.identity) << 64) | u128::from(queue.identity)
}
#[cfg(feature = "rc")]
fn prepare_receive<C>(
    queue: &CommandQueue<C>,
    state: &mut CommandQueueState<C>,
) -> Result<(), EcsError> {
    let exhausted = || classified_error(ErrorKind::ResourceExhausted, "命令容量或序号耗尽");
    if let CommandCapacity::Bounded(limit) = queue.capacity {
        if state.pending.len() >= limit.get() {
            return Err(exhausted());
        }
    }
    state.next_command.checked_add(1).ok_or_else(exhausted)?;
    let total = state.pending.len().checked_add(1).ok_or_else(exhausted)?;
    // 保守覆盖全部未消费项未来显现，已显现项无需再次分配序号。
    let delayed = state.pending.len() - state.ready.len();
    let future = u64::try_from(delayed)
        .ok()
        .and_then(|n| n.checked_add(1))
        .ok_or_else(exhausted)?;
    state.next_ready.checked_add(future).ok_or_else(exhausted)?;
    state.ready.reserve(total - state.ready.len());
    state.pending.reserve(1);
    Ok(())
}
#[cfg(feature = "rc")]
fn publish_ready<C>(state: &mut CommandQueueState<C>, id: u64) {
    let sequence = state.next_ready;
    state.next_ready = sequence.checked_add(1).expect("接收时预留就绪序号");
    state.ready.push_back((sequence, id));
}
use crate::command::*;
use pi_ecs_types::error::EcsError;
impl WorldEpoch {
    pub(super) fn initial_epoch_inner() -> Self {
        Self(0)
    }
}
#[cfg(not(feature = "rc"))]
impl CommandHub {
    pub(super) fn new_inner() -> Result<Self, EcsError> {
        todo!("编号：T3，尚未实现， 非复用服务身份、时钟控制与类型目录初始化")
    }
    pub(super) fn close_inner(&mut self) {
        todo!("编号：T4，尚未实现， 关闭截点及已接收 payload 唯一清理")
    }
    pub(super) fn register_inner<C: Send + 'static>(
        &mut self,
        _capacity: CommandCapacity,
    ) -> Result<(), EcsError> {
        todo!("编号：T5，尚未实现， 命令队列登记")
    }
    pub(super) fn sender_inner<C: Send + 'static>(
        &self,
        _epoch: WorldEpoch,
    ) -> Result<CommandSender<C>, EcsError> {
        todo!("编号：T6，尚未实现， 指定代次发送端绑定")
    }
    pub(super) fn receiver_inner<C: Send + 'static>(
        &mut self,
        _epoch: WorldEpoch,
    ) -> Result<CommandReceiver<C>, EcsError> {
        todo!("编号：T7，尚未实现， 唯一消费资格领取")
    }
    pub(super) fn advance_inner(&mut self, _now: SimulationTick) -> Result<(), EcsError> {
        todo!("编号：T8，尚未实现， 定时可见性推进")
    }
}
#[cfg(not(feature = "rc"))]
impl<C: Send + 'static> CommandSender<C> {
    pub(super) fn send_inner(&self, _command: C) -> Result<(), CommandSendError<C>> {
        todo!("编号：T9，尚未实现")
    }
    pub(super) fn schedule_inner(
        &self,
        _deadline: SimulationTick,
        _command: C,
    ) -> Result<TimerKey, CommandSendError<C>> {
        todo!("编号：T10，尚未实现")
    }
    pub(super) fn cancel_inner(&self, _key: TimerKey) -> Result<bool, EcsError> {
        todo!("编号：T11，尚未实现")
    }
}
#[cfg(not(feature = "rc"))]
impl<C: Send + 'static> CommandReceiver<C> {
    pub(super) fn read_inner(&mut self) -> Result<CommandBatch<'_, C>, EcsError> {
        todo!("编号：T12，尚未实现")
    }
    pub(super) fn receiver_drop_inner(&mut self) {
        todo!("编号：T13，尚未实现")
    }
}
#[cfg(not(feature = "rc"))]
impl<C: Send + 'static> CommandBatch<'_, C> {
    pub(super) fn next_inner(&mut self) -> Option<C> {
        todo!("编号：T14，尚未实现")
    }
}

#[cfg(not(feature = "rc"))]
impl<C: Send + 'static> CommandSender<C> {
    pub(super) fn clone_inner(&self) -> Self {
        todo!("编号：T15，尚未实现， 原类型化队列与代次的句柄克隆")
    }
}
#[cfg(not(feature = "rc"))]
impl CommandHub {
    pub(super) fn drop_inner(&mut self) {
        todo!("编号：T16，尚未实现， 关闭截点及唯一 payload 清理的自动兜底")
    }
}
#[cfg(feature = "rc")]
impl super::CommandHub {
    pub(super) fn queue_status_inner<C: Send + 'static>(
        &self,
    ) -> Result<super::CommandQueueStatus, pi_ecs_types::error::EcsError> {
        let queue = self.queue::<C>(self.endpoint.control.epoch)?;
        let state = queue
            .state
            .try_borrow()
            .map_err(|_| classified_error(ErrorKind::InvalidState, "命令队列状态正在被独占借用"))?;
        Ok(super::CommandQueueStatus {
            ready: !state.ready.is_empty(),
            receiver_claim: state.occupied.then_some(state.last_receiver_claim),
        })
    }
}
#[cfg(feature = "rc")]
impl<C: Send + 'static> super::CommandReceiver<C> {
    pub(super) fn view_inner(&mut self) -> super::CommandRead<'_, C> {
        super::CommandRead { receiver: self }
    }
}

#[cfg(feature = "rc")]
impl super::CommandHub {
    pub(super) fn has_receiver_claims_inner(&self) -> bool {
        self.endpoint
            .queues
            .values()
            .any(|entry| (entry.receiver_claimed)(entry.typed.as_ref()))
    }
}
