//! Rolling back a deploy: only while the version deployed is still the one the act deployed.
//!
//! The rule the attic keeps for files, kept for `kijin-deploy`: an undo is a lease on what the
//! act left. A later deploy — by the human, or by another run — is the later writer, and it is
//! kept; the rollback is refused and says what is deployed now.

/// A deploy target, as `kijin-deploy` drives it.
pub trait Deployment {
    /// The version deployed to `env` now, as the target itself reports it.
    ///
    /// # Errors
    ///
    /// The target cannot be read.
    fn deployed(&self, env: &str) -> Result<String, String>;

    /// Deploy `version` to `env`.
    ///
    /// # Errors
    ///
    /// The deploy fails.
    fn deploy(&mut self, env: &str, version: &str) -> Result<(), String>;
}

/// A deploy as its post journals it: where, what was there before, and what it put there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployed {
    /// The environment.
    pub env: String,
    /// The version deployed before the act.
    pub before: String,
    /// The version the act deployed.
    pub ours: String,
}

/// A rollback's outcome.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rollback {
    /// The act's version was still deployed; the one before it is deployed again.
    RolledBack {
        /// The environment.
        env: String,
        /// The version deployed again.
        to: String,
    },
    /// Another version is deployed now; nothing was rolled back.
    Conflict {
        /// The environment.
        env: String,
        /// The version the act deployed.
        ours: String,
        /// The version deployed now.
        now: String,
    },
}

/// Roll `act` back, only if `env` still runs the version it deployed.
///
/// # Errors
///
/// The target cannot be read, or the deploy of the old version fails.
pub fn roll_back(target: &mut impl Deployment, act: &Deployed) -> Result<Rollback, String> {
    let now = target.deployed(&act.env)?;
    if now != act.ours {
        return Ok(Rollback::Conflict {
            env: act.env.clone(),
            ours: act.ours.clone(),
            now,
        });
    }
    target.deploy(&act.env, &act.before)?;
    Ok(Rollback::RolledBack {
        env: act.env.clone(),
        to: act.before.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Deployed, Deployment, Rollback, roll_back};

    struct Fake {
        version: String,
        deploys: Vec<String>,
    }

    impl Deployment for Fake {
        fn deployed(&self, _env: &str) -> Result<String, String> {
            Ok(self.version.clone())
        }
        fn deploy(&mut self, _env: &str, version: &str) -> Result<(), String> {
            self.version = version.to_owned();
            self.deploys.push(version.to_owned());
            Ok(())
        }
    }

    fn act() -> Deployed {
        Deployed {
            env: "prod".into(),
            before: "v1".into(),
            ours: "v2".into(),
        }
    }

    #[test]
    fn our_version_still_deployed_rolls_back() {
        let mut t = Fake {
            version: "v2".into(),
            deploys: Vec::new(),
        };
        assert_eq!(
            roll_back(&mut t, &act()).unwrap(),
            Rollback::RolledBack {
                env: "prod".into(),
                to: "v1".into()
            }
        );
        assert_eq!(t.deploys, ["v1"]);
    }

    #[test]
    fn a_later_deploy_is_kept() {
        let mut t = Fake {
            version: "v3".into(),
            deploys: Vec::new(),
        };
        assert_eq!(
            roll_back(&mut t, &act()).unwrap(),
            Rollback::Conflict {
                env: "prod".into(),
                ours: "v2".into(),
                now: "v3".into()
            }
        );
        assert!(t.deploys.is_empty(), "nothing was deployed");
        assert_eq!(t.version, "v3");
    }
}
