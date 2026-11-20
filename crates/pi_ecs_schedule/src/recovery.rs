//! 完整游戏恢复协调；core 不反向依赖系统运行时。
//! @to_design 下列单 Schedule/&mut World 入口是待迁移兼容壳；正式 App 路径覆盖全部 Stage 一次，
//! 通过 World 内部维护门禁独占内部状态，不由共享 World 强转整个可变引用。
#[path = "recovery_inner.rs"]
mod recovery_inner;
use crate::{schedule::Schedule, snapshot::GameSnapshot};
use pi_ecs_core::{error::EcsError, world::World};
/// 游戏捕获恢复的编排入口，协调世界数据与系统状态。
pub struct GameRecovery;
impl GameRecovery {
    /// 捕获整个游戏 World 及对应系统状态的独立快照，不改变当前状态，不包含真实渲染数据。
    ///
    /// world 为借用的数据世界，schedule 为对应系统运行状态；捕获不修改两者。
    /// # Returns
    /// 成功返回可重复恢复的不可变快照；不要求快照自身实现 Clone 或序列化。
    ///
    /// # Errors
    /// 规则缺失或复制失败时清理已构造副本，不交付不完整快照，当前游戏状态不变。
    /// 捕获要求阶段任务、借用和 guard 已结束，待销毁实体已收束；不强制先 settle。
    /// 身份强弱计数原样捕获；调用方负责保存范围、派生初值和外部责任与该计数一致。
    /// 运行状态范围见 [GameSnapshot]；通知流与游标一同捕获，不在恢复时清空或移至流尾。
    pub fn capture_game(world: &World, schedule: &Schedule) -> Result<GameSnapshot, EcsError> {
        /* 设计已确认：静止点及独立历史；运行态分类见 GameSnapshot，存储实现尚未完成。
        ```mermaid
        flowchart TD
          A[检查静止点和保存规则] --> B{通过?}
          B -->|否| E[返回错误 当前状态不变]
          B -->|是| C[复制游戏 World 映射 原型行列与保存资源]
          C -->|失败| D[清理已构造副本]
          D --> E
          C -->|成功| F[返回独立快照 不含渲染]
        ```
        */
        Self::capture_game_inner(world, schedule)
    }

    /// 借用快照恢复整个游戏 World；独立渲染 World 不在此入口的操作范围，重演后由桥接协调。
    ///
    /// # Arguments
    /// * `world` - 原游戏 World 或已完成兼容登记的空 World，不包含另一个渲染 World。
    /// 目标须有一致的组件/Table 编号布局、分区配置和保存规则；首版不做任意编号重映射。
    /// * `schedule` - 兼容系统定义及参数/订阅配置，恢复 last_run、保存的 Local 和读取进度。
    /// * `snapshot` - 与身份配置、类型布局及登记规则兼容的快照；方法不消耗它。
    /// 调用方保证外部强弱句柄及 forget 释放责任与恢复后的原样计数一致；
    /// 简单用法为恢复前清偿外部拥有责任，恢复后重新取得。裸 Entity 的非拥有副本不受此限制。
    ///
    /// # Returns
    /// 成功后游戏状态及派生数据可用于后续模拟；不表示最终表现协调已执行。
    ///
    /// # Errors
    /// 校验和准备失败不替换当前游戏状态；开始安装或重建后失败停止相关模拟，
    /// 不承诺整体撤销。安装不逐实体触发普通生命周期通知，但仍正确析构和修复内部状态。
    /// 新数据先独立准备；旧值清理时仅针对旧计数，随后安装快照原样计数并激活目标绑定。
    /// 不扫描引用重建计数，不在重绑时逐引用增持；框架负责无重复析构和失败清理。
    /// @to_design 停止态表示、查询缓存与读取进度的配合、panic 边界尚未确定。
    pub fn restore_game(
        world: &mut World,
        schedule: &mut Schedule,
        snapshot: &GameSnapshot,
    ) -> Result<(), EcsError> {
        /* 设计已确认：以下阶段顺序及失败分界不表示实现已验证。
        ```mermaid
        flowchart TD
          A[校验兼容与重建依赖] -->|通过| B[准备独立副本与合法初始值]
          A -->|失败| X[返回错误 当前状态不变]
          B -->|失败| C[清理临时数据]
          C --> X
           B -->|成功| O[清理旧分支 仅操作旧计数 不递归归零销毁]
           O -->|失败| Y[停止模拟 不承诺撤销]
           O -->|成功| D[安装身份原样计数 原型与值 激活目标绑定]
           D -->|成功| E[推进 epoch 失效 Query Alter 访问计划]
          E -->|成功| F[按依赖重建]
          F -->|成功| G[允许继续模拟 快照不消耗]
          D -->|失败| Y[停止模拟 不承诺撤销]
          E -->|失败| Y
          F -->|失败| Y
        ```
        */
        Self::restore_game_inner(world, schedule, snapshot)
    }
}
