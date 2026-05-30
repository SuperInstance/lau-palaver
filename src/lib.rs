//! # lau-palaver
//!
//! Consensus protocol inspired by the West African palaver tree — where communities
//! make decisions through extended discussion until EVERYONE agrees. Not majority vote.
//! Not weighted vote. Consensus or we keep talking.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Unique identifier for a palaver session.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct PalaverId(String);

impl PalaverId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PalaverId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An agent's stance on a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Stance {
    Agree,
    Disagree(String),
    Abstain,
    Propose(String),
}

/// A single agent's position on a proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub agent_id: String,
    pub stance: Stance,
    pub confidence: f64,
    pub rationale: String,
    pub tick_stated: u64,
}

impl Position {
    pub fn new(agent_id: &str, stance: Stance, confidence: f64, rationale: &str, tick: u64) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            stance,
            confidence: confidence.clamp(0.0, 1.0),
            rationale: rationale.to_string(),
            tick_stated: tick,
        }
    }

    /// Returns true if this position represents agreement.
    pub fn is_agreement(&self) -> bool {
        matches!(self.stance, Stance::Agree)
    }
}

/// Status of a proposal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProposalStatus {
    Open,
    Consensus,
    Blocked,
    Withdrawn,
    Expired,
}

/// A proposal under discussion in a palaver.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub description: String,
    pub proposer: String,
    pub tick_proposed: u64,
    pub positions: HashMap<String, Position>,
    pub status: ProposalStatus,
    pub consensus_threshold: f64,
}

impl Proposal {
    pub fn new(id: &str, description: &str, proposer: &str, tick: u64) -> Self {
        Self {
            id: id.to_string(),
            description: description.to_string(),
            proposer: proposer.to_string(),
            tick_proposed: tick,
            positions: HashMap::new(),
            status: ProposalStatus::Open,
            consensus_threshold: 1.0,
        }
    }

    /// Add or update a position.
    pub fn state_position(&mut self, position: Position) {
        self.positions.insert(position.agent_id.clone(), position);
        self.update_status();
    }

    /// Withdraw an agent's position.
    pub fn withdraw_position(&mut self, agent_id: &str) {
        self.positions.remove(agent_id);
        self.update_status();
    }

    /// Fraction of positions that agree (out of total positions).
    pub fn consensus_score(&self) -> f64 {
        if self.positions.is_empty() {
            return 0.0;
        }
        let agreeing = self.positions.values().filter(|p| p.is_agreement()).count();
        agreeing as f64 / self.positions.len() as f64
    }

    /// True if consensus_score >= threshold.
    pub fn has_consensus(&self) -> bool {
        self.consensus_score() >= self.consensus_threshold
    }

    /// All positions that disagree.
    pub fn blockers(&self) -> Vec<&Position> {
        self.positions
            .values()
            .filter(|p| matches!(p.stance, Stance::Disagree(_)))
            .collect()
    }

    /// All positions that agree.
    pub fn proponents(&self) -> Vec<&Position> {
        self.positions
            .values()
            .filter(|p| p.is_agreement())
            .collect()
    }

    /// Any single agent can block.
    pub fn is_blocked(&self) -> bool {
        self.positions
            .values()
            .any(|p| matches!(p.stance, Stance::Disagree(_)))
    }

    /// Age of this proposal in ticks.
    pub fn tick_age(&self, current_tick: u64) -> u64 {
        current_tick.saturating_sub(self.tick_proposed)
    }

    fn update_status(&mut self) {
        // Only recalculate for non-terminal statuses
        if matches!(self.status, ProposalStatus::Withdrawn | ProposalStatus::Expired) {
            return;
        }
        if self.is_blocked() {
            self.status = ProposalStatus::Blocked;
        } else if self.has_consensus() {
            self.status = ProposalStatus::Consensus;
        } else {
            self.status = ProposalStatus::Open;
        }
    }
}

/// A palaver session — a discussion among agents under the palaver tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Palaver {
    pub id: PalaverId,
    pub topic: String,
    pub proposals: Vec<Proposal>,
    pub active_proposal: Option<usize>,
    pub participants: Vec<String>,
    pub tick_started: u64,
    pub tick_ended: Option<u64>,
    pub turn_count: u32,
    pub consensus_reached: bool,
    pub harmony_score: f64,
}

impl Palaver {
    pub fn new(topic: &str, participants: Vec<String>, tick: u64) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = PalaverId(format!("palaver-{}", COUNTER.fetch_add(1, Ordering::Relaxed)));
        Self {
            id,
            topic: topic.to_string(),
            proposals: Vec::new(),
            active_proposal: None,
            participants,
            tick_started: tick,
            tick_ended: None,
            turn_count: 0,
            consensus_reached: false,
            harmony_score: 0.0,
        }
    }

    /// Propose something. Returns the proposal index.
    pub fn propose(&mut self, description: &str, proposer: &str) -> usize {
        let idx = self.proposals.len();
        let proposal_id = format!("{}-prop-{}", self.id, idx);
        let proposal = Proposal::new(&proposal_id, description, proposer, self.tick_started + self.turn_count as u64);
        self.proposals.push(proposal);
        if self.active_proposal.is_none() {
            self.active_proposal = Some(idx);
        }
        self.turn_count += 1;
        idx
    }

    /// Respond to a proposal with a position.
    pub fn respond(&mut self, proposal_idx: usize, position: Position) -> Result<(), String> {
        if proposal_idx >= self.proposals.len() {
            return Err(format!("Proposal index {} out of bounds", proposal_idx));
        }
        let proposal = &mut self.proposals[proposal_idx];
        proposal.state_position(position);
        self.turn_count += 1;

        // Check if this proposal just reached consensus
        let proposal = &self.proposals[proposal_idx];
        self.consensus_reached = proposal.has_consensus() && !proposal.is_blocked();
        Ok(())
    }

    /// Check if the active proposal has consensus.
    pub fn check_consensus(&mut self) -> bool {
        if let Some(idx) = self.active_proposal {
            let proposal = &self.proposals[idx];
            proposal.has_consensus() && !proposal.is_blocked()
        } else {
            false
        }
    }

    /// Move to next proposal if current is blocked.
    pub fn advance_proposal(&mut self) {
        if let Some(idx) = self.active_proposal {
            if idx + 1 < self.proposals.len() {
                self.active_proposal = Some(idx + 1);
            }
        }
    }

    /// Get the current active proposal.
    pub fn current_proposal(&self) -> Option<&Proposal> {
        self.active_proposal.and_then(|idx| self.proposals.get(idx))
    }

    /// Get all proposals.
    pub fn all_proposals(&self) -> &[Proposal] {
        &self.proposals
    }

    /// Update harmony score from tensor MIDI.
    pub fn update_harmony(&mut self, score: f64) {
        self.harmony_score = score.clamp(0.0, 1.0);
    }

    /// Close the palaver if consensus was reached. Returns true if closed.
    pub fn close(&mut self, tick: u64) -> bool {
        if self.consensus_reached {
            self.tick_ended = Some(tick);
            true
        } else {
            false
        }
    }

    /// Human-readable summary.
    pub fn summary(&self) -> String {
        let status = if self.consensus_reached {
            "CONSENSUS REACHED"
        } else if self.tick_ended.is_some() {
            "CLOSED"
        } else {
            "IN DISCUSSION"
        };
        let proposal_summaries: Vec<String> = self
            .proposals
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "  [{}] \"{}\" by {} — {:?} (score: {:.0}%, {}/{} agree)",
                    i,
                    p.description,
                    p.proposer,
                    p.status,
                    p.consensus_score() * 100.0,
                    p.proponents().len(),
                    p.positions.len(),
                )
            })
            .collect();
        format!(
            "Palaver \"{}\" [{}]\n  Participants: {}\n  Turns: {}\n  Harmony: {:.0}%\n{}",
            self.topic,
            status,
            self.participants.join(", "),
            self.turn_count,
            self.harmony_score * 100.0,
            proposal_summaries.join("\n"),
        )
    }
}

/// Statistics across all palavers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalaverStats {
    pub total: usize,
    pub consensus_reached: usize,
    pub blocked: usize,
    pub active: usize,
    pub avg_turns_to_consensus: f64,
    pub avg_harmony_at_consensus: f64,
    pub consensus_rate: f64,
}

/// The palaver tree — manages multiple palaver sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalaverTree {
    palavers: HashMap<PalaverId, Palaver>,
    global_participants: HashSet<String>,
}

impl PalaverTree {
    pub fn new() -> Self {
        Self {
            palavers: HashMap::new(),
            global_participants: HashSet::new(),
        }
    }

    /// Open a new palaver session.
    pub fn open_palaver(&mut self, topic: &str, participants: Vec<String>, tick: u64) -> PalaverId {
        for p in &participants {
            self.global_participants.insert(p.clone());
        }
        let palaver = Palaver::new(topic, participants, tick);
        let id = palaver.id.clone();
        self.palavers.insert(id.clone(), palaver);
        id
    }

    /// Propose within a palaver.
    pub fn propose(&mut self, palaver_id: &PalaverId, description: &str, proposer: &str) -> Option<usize> {
        self.palavers.get_mut(palaver_id).map(|p| p.propose(description, proposer))
    }

    /// Respond to a proposal in a palaver.
    pub fn respond(&mut self, palaver_id: &PalaverId, proposal_idx: usize, position: Position) -> bool {
        if let Some(palaver) = self.palavers.get_mut(palaver_id) {
            palaver.respond(proposal_idx, position).is_ok()
        } else {
            false
        }
    }

    /// Check consensus in a palaver.
    pub fn check_consensus(&mut self, palaver_id: &PalaverId) -> bool {
        self.palavers.get_mut(palaver_id).map(|p| p.check_consensus()).unwrap_or(false)
    }

    /// Close a palaver.
    pub fn close_palaver(&mut self, palaver_id: &PalaverId, tick: u64) -> bool {
        self.palavers.get_mut(palaver_id).map(|p| p.close(tick)).unwrap_or(false)
    }

    /// Get all active (unresolved) palavers.
    pub fn active_palavers(&self) -> Vec<&Palaver> {
        self.palavers.values().filter(|p| p.tick_ended.is_none()).collect()
    }

    /// Get all resolved palavers.
    pub fn resolved_palavers(&self) -> Vec<&Palaver> {
        self.palavers.values().filter(|p| p.tick_ended.is_some()).collect()
    }

    /// Get all palavers an agent participates in.
    pub fn agent_palavers(&self, agent_id: &str) -> Vec<&Palaver> {
        self.palavers
            .values()
            .filter(|p| p.participants.contains(&agent_id.to_string()))
            .collect()
    }

    /// Overall consensus rate across all palavers.
    pub fn consensus_rate(&self) -> f64 {
        if self.palavers.is_empty() {
            return 0.0;
        }
        let reached = self.palavers.values().filter(|p| p.consensus_reached).count();
        reached as f64 / self.palavers.len() as f64
    }

    /// Aggregate statistics.
    pub fn tree_stats(&self) -> PalaverStats {
        let total = self.palavers.len();
        let consensus_reached = self.palavers.values().filter(|p| p.consensus_reached).count();
        let blocked = self
            .palavers
            .values()
            .filter(|p| p.proposals.iter().any(|pr| pr.status == ProposalStatus::Blocked))
            .count();
        let active = self.palavers.values().filter(|p| p.tick_ended.is_none()).count();

        let consensus_palavers: Vec<&Palaver> = self
            .palavers
            .values()
            .filter(|p| p.consensus_reached)
            .collect();

        let avg_turns = if consensus_palavers.is_empty() {
            0.0
        } else {
            consensus_palavers.iter().map(|p| p.turn_count as f64).sum::<f64>()
                / consensus_palavers.len() as f64
        };

        let avg_harmony = if consensus_palavers.is_empty() {
            0.0
        } else {
            consensus_palavers.iter().map(|p| p.harmony_score).sum::<f64>()
                / consensus_palavers.len() as f64
        };

        PalaverStats {
            total,
            consensus_reached,
            blocked,
            active,
            avg_turns_to_consensus: avg_turns,
            avg_harmony_at_consensus: avg_harmony,
            consensus_rate: self.consensus_rate(),
        }
    }
}

impl Default for PalaverTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_position(agent: &str, stance: Stance, confidence: f64, tick: u64) -> Position {
        Position::new(agent, stance, confidence, "test rationale", tick)
    }

    #[test]
    fn test_palaver_id_clone_hash_eq() {
        let id1 = PalaverId("p1".to_string());
        let id2 = id1.clone();
        assert_eq!(id1, id2);
        let mut set = HashSet::new();
        set.insert(id1.clone());
        assert!(set.contains(&id1));
    }

    #[test]
    fn test_stance_agreement() {
        assert!(make_position("a", Stance::Agree, 1.0, 0).is_agreement());
        assert!(!make_position("a", Stance::Disagree("no".into()), 0.5, 0).is_agreement());
        assert!(!make_position("a", Stance::Abstain, 0.5, 0).is_agreement());
        assert!(!make_position("a", Stance::Propose("alt".into()), 0.5, 0).is_agreement());
    }

    #[test]
    fn test_position_confidence_clamped() {
        let p = Position::new("a", Stance::Agree, 2.0, "r", 0);
        assert!((p.confidence - 1.0).abs() < f64::EPSILON);
        let p = Position::new("a", Stance::Agree, -1.0, "r", 0);
        assert!((p.confidence).abs() < f64::EPSILON);
    }

    #[test]
    fn test_proposal_new() {
        let p = Proposal::new("id", "desc", "proposer", 10);
        assert_eq!(p.id, "id");
        assert_eq!(p.description, "desc");
        assert_eq!(p.proposer, "proposer");
        assert_eq!(p.tick_proposed, 10);
        assert_eq!(p.status, ProposalStatus::Open);
        assert!((p.consensus_threshold - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_proposal_state_position_agree() {
        let mut p = Proposal::new("id", "desc", "proposer", 0);
        p.state_position(make_position("a", Stance::Agree, 1.0, 1));
        assert_eq!(p.positions.len(), 1);
        assert!((p.consensus_score() - 1.0).abs() < f64::EPSILON);
        assert!(p.has_consensus());
        assert_eq!(p.status, ProposalStatus::Consensus);
    }

    #[test]
    fn test_proposal_state_position_disagree() {
        let mut p = Proposal::new("id", "desc", "proposer", 0);
        p.state_position(make_position("a", Stance::Disagree("no".into()), 0.9, 1));
        assert!(p.is_blocked());
        assert_eq!(p.status, ProposalStatus::Blocked);
        assert_eq!(p.blockers().len(), 1);
    }

    #[test]
    fn test_proposal_withdraw() {
        let mut p = Proposal::new("id", "desc", "proposer", 0);
        p.state_position(make_position("a", Stance::Disagree("no".into()), 0.9, 1));
        assert_eq!(p.status, ProposalStatus::Blocked);
        p.withdraw_position("a");
        assert!(p.positions.is_empty());
        // Withdrawn disagree reverts to Open (no positions, no blockers)
        assert_eq!(p.status, ProposalStatus::Open);
    }

    #[test]
    fn test_consensus_score_partial() {
        let mut p = Proposal::new("id", "desc", "prop", 0);
        p.state_position(make_position("a", Stance::Agree, 1.0, 1));
        p.state_position(make_position("b", Stance::Disagree("no".into()), 0.8, 1));
        assert!((p.consensus_score() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_blockers_and_proponents() {
        let mut p = Proposal::new("id", "desc", "prop", 0);
        // Manually set status back to Open after adding disagree to test mixed
        p.state_position(make_position("a", Stance::Agree, 1.0, 1));
        p.state_position(make_position("b", Stance::Disagree("no".into()), 0.8, 2));
        p.state_position(make_position("c", Stance::Abstain, 0.5, 3));
        assert_eq!(p.proponents().len(), 1);
        assert_eq!(p.blockers().len(), 1);
    }

    #[test]
    fn test_tick_age() {
        let p = Proposal::new("id", "desc", "prop", 10);
        assert_eq!(p.tick_age(20), 10);
        assert_eq!(p.tick_age(5), 0); // saturating
    }

    #[test]
    fn test_consensus_threshold_custom() {
        let mut p = Proposal::new("id", "desc", "prop", 0);
        p.consensus_threshold = 0.5;
        p.state_position(make_position("a", Stance::Agree, 1.0, 1));
        p.state_position(make_position("b", Stance::Disagree("no".into()), 0.8, 2));
        // 1 agree out of 2 = 0.5 >= 0.5 threshold but also blocked
        assert!(p.has_consensus());
        assert!(p.is_blocked());
    }

    #[test]
    fn test_palaver_new() {
        let p = Palaver::new("topic", vec!["a".into(), "b".into()], 0);
        assert_eq!(p.topic, "topic");
        assert_eq!(p.participants.len(), 2);
        assert_eq!(p.tick_started, 0);
        assert!(p.active_proposal.is_none());
        assert!(!p.consensus_reached);
    }

    #[test]
    fn test_palaver_propose() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        let idx = p.propose("do the thing", "a");
        assert_eq!(idx, 0);
        assert_eq!(p.active_proposal, Some(0));
        assert_eq!(p.proposals.len(), 1);
        assert_eq!(p.turn_count, 1);
    }

    #[test]
    fn test_palaver_respond() {
        let mut p = Palaver::new("topic", vec!["a".into(), "b".into()], 0);
        let idx = p.propose("do the thing", "a");
        p.respond(idx, make_position("a", Stance::Agree, 1.0, 1)).unwrap();
        p.respond(idx, make_position("b", Stance::Agree, 0.9, 2)).unwrap();
        assert!(p.consensus_reached);
    }

    #[test]
    fn test_palaver_respond_blocked() {
        let mut p = Palaver::new("topic", vec!["a".into(), "b".into()], 0);
        let idx = p.propose("do the thing", "a");
        p.respond(idx, make_position("a", Stance::Agree, 1.0, 1)).unwrap();
        p.respond(idx, make_position("b", Stance::Disagree("nope".into()), 0.9, 2)).unwrap();
        assert!(!p.consensus_reached);
    }

    #[test]
    fn test_palaver_respond_out_of_bounds() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        let result = p.respond(99, make_position("a", Stance::Agree, 1.0, 1));
        assert!(result.is_err());
    }

    #[test]
    fn test_palaver_check_consensus() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        assert!(!p.check_consensus()); // no proposal
        let idx = p.propose("thing", "a");
        p.respond(idx, make_position("a", Stance::Agree, 1.0, 1)).unwrap();
        assert!(p.check_consensus());
    }

    #[test]
    fn test_palaver_advance_proposal() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        let i0 = p.propose("first", "a");
        let _i1 = p.propose("second", "a");
        assert_eq!(p.active_proposal, Some(i0));
        p.advance_proposal();
        assert_eq!(p.active_proposal, Some(1));
    }

    #[test]
    fn test_palaver_current_proposal() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        assert!(p.current_proposal().is_none());
        p.propose("thing", "a");
        assert!(p.current_proposal().is_some());
        assert_eq!(p.current_proposal().unwrap().description, "thing");
    }

    #[test]
    fn test_palaver_all_proposals() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        p.propose("first", "a");
        p.propose("second", "a");
        assert_eq!(p.all_proposals().len(), 2);
    }

    #[test]
    fn test_palaver_update_harmony() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        p.update_harmony(0.75);
        assert!((p.harmony_score - 0.75).abs() < f64::EPSILON);
        p.update_harmony(2.0);
        assert!((p.harmony_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_palaver_close() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        assert!(!p.close(10)); // no consensus
        let idx = p.propose("thing", "a");
        p.respond(idx, make_position("a", Stance::Agree, 1.0, 1)).unwrap();
        assert!(p.close(10));
        assert_eq!(p.tick_ended, Some(10));
    }

    #[test]
    fn test_palaver_summary() {
        let mut p = Palaver::new("build the thing", vec!["a".into(), "b".into()], 0);
        p.propose("use rust", "a");
        let summary = p.summary();
        assert!(summary.contains("build the thing"));
        assert!(summary.contains("IN DISCUSSION"));
    }

    #[test]
    fn test_palaver_tree_open() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver("topic", vec!["a".into(), "b".into()], 0);
        assert!(!id.as_str().is_empty());
        assert_eq!(tree.active_palavers().len(), 1);
    }

    #[test]
    fn test_palaver_tree_propose_and_respond() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver("topic", vec!["a".into()], 0);
        let idx = tree.propose(&id, "do it", "a").unwrap();
        assert!(tree.respond(&id, idx, make_position("a", Stance::Agree, 1.0, 1)));
        assert!(tree.check_consensus(&id));
    }

    #[test]
    fn test_palaver_tree_close() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver("topic", vec!["a".into()], 0);
        let idx = tree.propose(&id, "do it", "a").unwrap();
        tree.respond(&id, idx, make_position("a", Stance::Agree, 1.0, 1));
        assert!(tree.close_palaver(&id, 10));
        assert_eq!(tree.resolved_palavers().len(), 1);
        assert_eq!(tree.active_palavers().len(), 0);
    }

    #[test]
    fn test_palaver_tree_agent_palavers() {
        let mut tree = PalaverTree::new();
        let id1 = tree.open_palaver("t1", vec!["a".into(), "b".into()], 0);
        let _id2 = tree.open_palaver("t2", vec!["b".into(), "c".into()], 0);
        assert_eq!(tree.agent_palavers("a").len(), 1);
        assert_eq!(tree.agent_palavers("b").len(), 2);
        assert_eq!(tree.agent_palavers("c").len(), 1);
        assert_eq!(tree.agent_palavers("d").len(), 0);
        // use id1 to suppress warning
        let _ = &id1;
    }

    #[test]
    fn test_palaver_tree_consensus_rate() {
        let mut tree = PalaverTree::new();
        assert!((tree.consensus_rate()).abs() < f64::EPSILON);

        let id1 = tree.open_palaver("t1", vec!["a".into()], 0);
        let idx1 = tree.propose(&id1, "p1", "a").unwrap();
        tree.respond(&id1, idx1, make_position("a", Stance::Agree, 1.0, 1));

        let _id2 = tree.open_palaver("t2", vec!["b".into()], 0);
        // only t1 has consensus
        assert!((tree.consensus_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_palaver_tree_stats() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver("t1", vec!["a".into()], 0);
        let idx = tree.propose(&id, "p1", "a").unwrap();
        tree.respond(&id, idx, make_position("a", Stance::Agree, 1.0, 1));
        tree.close_palaver(&id, 10);

        let stats = tree.tree_stats();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.consensus_reached, 1);
        assert_eq!(stats.active, 0);
        assert!((stats.consensus_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_palaver_tree_stats_multiple() {
        let mut tree = PalaverTree::new();

        // Consensus palaver
        let id1 = tree.open_palaver("t1", vec!["a".into()], 0);
        let idx1 = tree.propose(&id1, "p1", "a").unwrap();
        tree.respond(&id1, idx1, make_position("a", Stance::Agree, 1.0, 1));
        tree.close_palaver(&id1, 5);

        // Blocked palaver
        let id2 = tree.open_palaver("t2", vec!["a".into(), "b".into()], 0);
        let idx2 = tree.propose(&id2, "p2", "a").unwrap();
        tree.respond(&id2, idx2, make_position("a", Stance::Disagree("no".into()), 1.0, 1));

        let stats = tree.tree_stats();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.consensus_reached, 1);
        assert_eq!(stats.blocked, 1);
        assert_eq!(stats.active, 1);
    }

    #[test]
    fn test_palaver_tree_default() {
        let tree = PalaverTree::default();
        assert_eq!(tree.active_palavers().len(), 0);
    }

    #[test]
    fn test_palaver_tree_invalid_id() {
        let mut tree = PalaverTree::new();
        let fake = PalaverId("fake".into());
        assert!(tree.propose(&fake, "x", "a").is_none());
        assert!(!tree.respond(&fake, 0, make_position("a", Stance::Agree, 1.0, 1)));
        assert!(!tree.check_consensus(&fake));
        assert!(!tree.close_palaver(&fake, 10));
    }

    #[test]
    fn test_full_consensus_flow() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver(
            "What language for the new service?",
            vec!["alice".into(), "bob".into(), "carol".into()],
            100,
        );

        let idx = tree.propose(&id, "Use Rust", "alice").unwrap();

        // Alice agrees — 1/1 = 100% consensus among stated positions
        tree.respond(&id, idx, make_position("alice", Stance::Agree, 0.95, 101));
        assert!(tree.check_consensus(&id));

        // Bob disagrees — now 1/2 agree = blocked
        tree.respond(&id, idx, make_position("bob", Stance::Disagree("Too complex".into()), 0.8, 102));
        assert!(!tree.check_consensus(&id));

        // Bob changes mind — 2/2 agree = consensus
        tree.respond(&id, idx, make_position("bob", Stance::Agree, 0.7, 105));
        assert!(tree.check_consensus(&id));

        // Carol agrees — still consensus
        tree.respond(&id, idx, make_position("carol", Stance::Agree, 0.9, 106));
        assert!(tree.check_consensus(&id));

        assert!(tree.close_palaver(&id, 110));

        let stats = tree.tree_stats();
        assert!((stats.consensus_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut tree = PalaverTree::new();
        let id = tree.open_palaver("topic", vec!["a".into()], 0);
        let idx = tree.propose(&id, "p", "a").unwrap();
        tree.respond(&id, idx, make_position("a", Stance::Agree, 1.0, 1));

        let json = serde_json::to_string(&tree).unwrap();
        let back: PalaverTree = serde_json::from_str(&json).unwrap();
        assert_eq!(back.active_palavers().len(), tree.active_palavers().len());
    }

    #[test]
    fn test_proposal_tick_age_zero() {
        let p = Proposal::new("id", "desc", "prop", 10);
        assert_eq!(p.tick_age(10), 0);
    }

    #[test]
    fn test_stance_equality() {
        assert_eq!(Stance::Agree, Stance::Agree);
        assert_eq!(Stance::Disagree("reason".into()), Stance::Disagree("reason".into()));
        assert_ne!(Stance::Agree, Stance::Abstain);
    }

    #[test]
    fn test_palaver_tree_global_participants() {
        let mut tree = PalaverTree::new();
        tree.open_palaver("t1", vec!["a".into(), "b".into()], 0);
        tree.open_palaver("t2", vec!["b".into(), "c".into()], 0);
        assert!(tree.global_participants.contains("a"));
        assert!(tree.global_participants.contains("b"));
        assert!(tree.global_participants.contains("c"));
        assert_eq!(tree.global_participants.len(), 3);
    }

    #[test]
    fn test_palaver_update_harmony_clamp() {
        let mut p = Palaver::new("topic", vec!["a".into()], 0);
        p.update_harmony(-0.5);
        assert!((p.harmony_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_proposal_consensus() {
        let p = Proposal::new("id", "desc", "prop", 0);
        assert!((p.consensus_score()).abs() < f64::EPSILON);
        assert!(!p.has_consensus());
    }

    #[test]
    fn test_abstain_does_not_block() {
        let mut p = Proposal::new("id", "desc", "prop", 0);
        p.consensus_threshold = 0.5;
        p.state_position(make_position("a", Stance::Agree, 1.0, 1));
        p.state_position(make_position("b", Stance::Abstain, 0.5, 1));
        // 1 agree out of 2 = 0.5, threshold is 0.5
        assert!(p.has_consensus());
        assert!(!p.is_blocked());
        assert_eq!(p.status, ProposalStatus::Consensus);
    }
}
