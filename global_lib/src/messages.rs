use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::{as_number, Step};

as_number!(
    u8,
    enum NodeCommand {
        Setup,
        Key,
        Kill,
        Process,
        Clean,
        Summary,
    },
    derive(Debug)
);

as_number!(
    u8,
    enum HavenCommand {
        Send,
        Echo,
        Ready,
    },
    derive(Debug)
);

as_number!(
    u8,
    enum AvssSimplCommand {
        Share,
        Ack,
        NewShare,
    },
    derive(Debug)
);

as_number!(
    u8,
    enum OneSidedVoteCommand {
        Ok,
        Vote,
    }
);

as_number!(
    u8,
    enum BadgerCommand {
        ReconstructShare,
    }
);

as_number!(
    u8,
    enum BroadcastCommand {
        Propose,
        Echo,
        Ready,
    },
    derive(Debug)
);

as_number!(
    u8,
    enum DispRetCommand {
        Propose,
        Echo,
        Ready,
    }
);

as_number!(
    u8,
    enum SecureMsgDisCommand {
        Propose,
        Echo,
        Vote,
        Forward,
    }
);

as_number!(
    u8,
    enum BeaconCommand {
        Dummy,
    }
);

as_number!(
    u8,
    enum BingoCommand {
        MyLine,
        NewRow,
        NewCol,
        NewDone,
        ReconstructShare,
    }
);

as_number!(
    u8,
    enum HbAvssCommand {
        Complaint,
        Assist,
    }
);

as_number!(
    u8,
    enum NameSpace {
        Heart,
        Broadcast,
        SecureMsgDis,
        AvssSimpl,
        Bingo,
        LightWeight,
        Badger,
        HbAvss,
        Haven,
        OneSidedVote,
        DisperseRetrieve,
    },
    derive(Clone, Copy, Debug)
);

as_number!(
    u8,
    enum ManagerCode {
        Gen,
        Connect,
    }
);

as_number!(
    u8,
    enum InterfaceCode {
        Connect,
        Output,
        NodeReady,
        PoolCleaned,
    },
    derive(Debug)
);

#[derive(Debug, PartialEq, Default, Serialize, Deserialize, Clone, Copy, Hash, Eq)]
pub enum Algo {
    #[default]
    AvssSimpl,
    DualAvssSimpl,
    Bingo,
    LightWeight,
    Badger,
    HbAvss,
    Haven,
}

impl From<Algo> for NameSpace {
    fn from(algo: Algo) -> NameSpace {
        match algo {
            Algo::AvssSimpl | Algo::DualAvssSimpl => NameSpace::AvssSimpl,
            Algo::Bingo => NameSpace::Bingo,
            Algo::LightWeight => NameSpace::LightWeight,
            Algo::Badger => NameSpace::Badger,
            Algo::HbAvss => NameSpace::HbAvss,
            Algo::Haven => NameSpace::Haven,
        }
    }
}

impl Algo {
    pub fn all() -> Vec<Self> {
        vec![Self::AvssSimpl, Self::Bingo, Self::LightWeight]
    }

    pub fn domain_size(self, n: u16, b: u16) -> u16 {
        match self {
            Self::Bingo => n + b,
            _ => n,
        }
    }

    pub fn curve_color(&self) -> &'static str {
        match self {
            Algo::Haven => "goldenrod",
            Algo::Bingo => "red",
            Algo::LightWeight => "green",
            Algo::AvssSimpl => "cyan",
            Algo::DualAvssSimpl => "midnight-blue",
            Algo::HbAvss => "purple",
            Algo::Badger => "black",
        }
    }

    pub fn support(&self, step: Step) -> bool {
        self.step_supported().contains(&step)
    }

    pub fn step_supported(&self) -> Vec<Step> {
        match self {
            Algo::Badger | Algo::Bingo | Algo::AvssSimpl | Algo::DualAvssSimpl => {
                vec![Step::Sharing, Step::Reconstruct]
            }
            Algo::LightWeight | Algo::HbAvss | Algo::Haven => vec![Step::Sharing],
        }
    }

    pub fn get_subprotocols(self) -> Vec<NameSpace> {
        match self {
            Algo::Haven => vec![],
            Algo::Bingo | Algo::AvssSimpl | Algo::Badger | Algo::DualAvssSimpl => {
                vec![NameSpace::Broadcast]
            }
            Algo::HbAvss => vec![
                NameSpace::Broadcast,
                NameSpace::OneSidedVote,
                NameSpace::DisperseRetrieve,
            ],
            Algo::LightWeight => vec![
                NameSpace::Broadcast,
                NameSpace::OneSidedVote,
                NameSpace::SecureMsgDis,
            ],
        }
    }
}

impl From<Algo> for &'static str {
    fn from(v: Algo) -> &'static str {
        match v {
            Algo::AvssSimpl => "AvssSimpl",
            Algo::DualAvssSimpl => "AvssSimpl, Dual Threshold",
            Algo::Bingo => "Bingo",
            Algo::LightWeight => "Lightweight",
            Algo::Badger => "Honey Badger",
            Algo::HbAvss => "hbACSS",
            Algo::Haven => "Haven",
        }
    }
}

impl Display for Algo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{}", Into::<&'static str>::into(*self))
    }
}

impl From<&str> for Algo {
    fn from(s: &str) -> Self {
        match s {
            "haven" => Self::Haven,
            "bingo" => Self::Bingo,
            "avss_simpl" => Self::AvssSimpl,
            "dual_avss_simpl" => Self::DualAvssSimpl,
            "lightweight" => Algo::LightWeight,
            "badger" => Algo::Badger,
            "hbacss" => Algo::HbAvss,
            _ => panic!("Algo doesn't exists"),
        }
    }
}
