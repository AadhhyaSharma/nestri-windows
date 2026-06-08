use clap::ValueEnum;

#[derive(Debug, Eq, PartialEq, Clone, ValueEnum, Default)]
pub enum LatencyControl {
    #[default]
    LowestLatency,
    HighestQuality,
}

#[derive(Debug, Clone)]
pub struct CbrConfig   { pub target_bitrate: u32 }
#[derive(Debug, Clone)]
pub struct VbrConfig   { pub target_bitrate: u32, pub max_bitrate: u32 }
#[derive(Debug, Clone)]
pub struct CqpConfig   { pub quality: u32 }

#[derive(Debug, Clone)]
pub enum RateControl {
    CBR(CbrConfig),
    VBR(VbrConfig),
    CQP(CqpConfig),
}

impl RateControl {
    /// Parse a rate control string like "cbr:8000", "vbr:6000:12000", "cqp:28"
    pub fn parse(s: &str) -> Self {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.as_slice() {
            ["cbr", bps]           => RateControl::CBR(CbrConfig { target_bitrate: bps.parse().unwrap_or(8000) }),
            ["vbr", t, m]          => RateControl::VBR(VbrConfig {
                target_bitrate: t.parse().unwrap_or(6000),
                max_bitrate:    m.parse().unwrap_or(12000),
            }),
            ["cqp", q]             => RateControl::CQP(CqpConfig { quality: q.parse().unwrap_or(28) }),
            _                      => RateControl::CBR(CbrConfig { target_bitrate: 8000 }),
        }
    }
}
