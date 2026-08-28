use crate::domain::PipelineStage;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// 全局流水线运行状态管理器。用一张并发集合替代散落的 AtomicBool。
#[derive(Clone, Default)]
pub struct PipelineManager {
    running: Arc<RwLock<HashSet<PipelineStage>>>,
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    fn read_set(&self) -> HashSet<PipelineStage> {
        self.running
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// 判断某阶段是否正在运行。
    pub fn is_running(&self, stage: PipelineStage) -> bool {
        self.running
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&stage)
    }

    /// 启动某阶段。返回 `true` 表示本次成功切入运行态。
    pub fn start(&self, stage: PipelineStage) -> bool {
        self.running
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(stage)
    }

    /// 终止某阶段，自动重置为待机。返回 `true` 表示原先正在运行。
    pub fn stop(&self, stage: PipelineStage) -> bool {
        self.running
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&stage)
    }

    /// 当前所有正在运行的阶段（按流水线时序排列）。
    pub fn running_stages(&self) -> Vec<PipelineStage> {
        let set = self.read_set();
        PipelineStage::ALL
            .into_iter()
            .filter(|stage| set.contains(stage))
            .collect()
    }
}
