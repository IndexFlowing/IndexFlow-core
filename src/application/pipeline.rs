use crate::domain::PipelineStage;
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// 站点维度的全站流水线运行状态管理器。
#[derive(Clone, Default)]
pub struct PipelineManager {
    running: Arc<RwLock<HashSet<(i64, PipelineStage)>>>,
}

impl PipelineManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    fn read_set(&self) -> HashSet<(i64, PipelineStage)> {
        self.running
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// 判断指定站点的某阶段是否正在运行。
    pub fn is_running(&self, site_id: i64, stage: PipelineStage) -> bool {
        self.running
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&(site_id, stage))
    }

    /// 启动指定站点的某阶段。返回 `true` 表示本次成功切入运行态。
    pub fn start(&self, site_id: i64, stage: PipelineStage) -> bool {
        self.running
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert((site_id, stage))
    }

    /// 终止指定站点的某阶段，自动重置为待机。返回 `true` 表示原先正在运行。
    pub fn stop(&self, site_id: i64, stage: PipelineStage) -> bool {
        self.running
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&(site_id, stage))
    }

    /// 查询某阶段正在运行的所有 site_id（去重）。
    pub fn running_sites_for_stage(&self, stage: PipelineStage) -> Vec<i64> {
        let set = self.read_set();
        let mut sites: Vec<i64> = set
            .into_iter()
            .filter_map(|(s_id, s_stage)| if s_stage == stage { Some(s_id) } else { None })
            .collect();
        sites.sort_unstable();
        sites.dedup();
        sites
    }

    /// 当前指定站点所有正在运行的阶段（按流水线时序排列）。
    pub fn running_stages(&self, site_id: i64) -> Vec<PipelineStage> {
        let set = self.read_set();
        PipelineStage::ALL
            .into_iter()
            .filter(|stage| set.contains(&(site_id, *stage)))
            .collect()
    }
}