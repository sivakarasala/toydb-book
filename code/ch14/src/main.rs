/// Chapter 14: Raft — Leader Election
/// Exercise: Build a Raft election state machine.

#[derive(Debug, Clone, PartialEq)]
pub enum NodeState { Follower, Candidate, Leader }

pub struct RaftNode {
    pub id: u64,
    pub state: NodeState,
    pub current_term: u64,
    pub voted_for: Option<u64>,
    pub votes_received: usize,
    pub peer_count: usize,
}

#[derive(Debug)]
pub struct RequestVote { pub term: u64, pub candidate_id: u64 }

#[derive(Debug)]
pub struct VoteResponse { pub term: u64, pub granted: bool }

impl RaftNode {
    pub fn new(id: u64, peer_count: usize) -> Self {
        RaftNode { id, state: NodeState::Follower, current_term: 0, voted_for: None, votes_received: 0, peer_count }
    }

    /// Start an election: become Candidate, increment term, vote for self.
    pub fn start_election(&mut self) {
        // TODO: Set state to Candidate, increment term, vote for self, set votes_received to 1
        todo!("Implement start_election")
    }

    /// Handle a RequestVote from another node.
    pub fn handle_vote_request(&mut self, req: &RequestVote) -> VoteResponse {
        // TODO:
        // If req.term > current_term: update term, become Follower, clear voted_for
        // Grant vote if: haven't voted this term (or already voted for this candidate) AND req.term >= current_term
        todo!("Implement handle_vote_request")
    }

    /// Handle a VoteResponse.
    pub fn handle_vote_response(&mut self, resp: &VoteResponse) {
        // TODO:
        // If resp.term > current_term: become Follower
        // If granted and still Candidate: increment votes_received
        // If votes_received > (peer_count + 1) / 2: become Leader
        todo!("Implement handle_vote_response")
    }

    fn majority(&self) -> usize { (self.peer_count + 1) / 2 + 1 }
}

fn main() {
    println!("=== Chapter 14: Raft Leader Election ===");
    println!("Run `cargo test --bin exercise` to check.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let node = RaftNode::new(1, 4);
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 0);
    }

    #[test]
    fn test_start_election() {
        let mut node = RaftNode::new(1, 4);
        node.start_election();
        assert_eq!(node.state, NodeState::Candidate);
        assert_eq!(node.current_term, 1);
        assert_eq!(node.voted_for, Some(1));
        assert_eq!(node.votes_received, 1);
    }

    #[test]
    fn test_win_election() {
        let mut node = RaftNode::new(1, 4);
        node.start_election(); // votes=1, need 3 of 5
        node.handle_vote_response(&VoteResponse { term: 1, granted: true }); // votes=2
        assert_eq!(node.state, NodeState::Candidate);
        node.handle_vote_response(&VoteResponse { term: 1, granted: true }); // votes=3 → Leader!
        assert_eq!(node.state, NodeState::Leader);
    }

    #[test]
    fn test_higher_term_steps_down() {
        let mut node = RaftNode::new(1, 4);
        node.start_election();
        node.handle_vote_response(&VoteResponse { term: 5, granted: false });
        assert_eq!(node.state, NodeState::Follower);
        assert_eq!(node.current_term, 5);
    }

    #[test]
    fn test_vote_granting() {
        let mut node = RaftNode::new(2, 4);
        let resp = node.handle_vote_request(&RequestVote { term: 1, candidate_id: 1 });
        assert!(resp.granted);
        assert_eq!(node.voted_for, Some(1));
        // Can't vote for someone else in same term
        let resp2 = node.handle_vote_request(&RequestVote { term: 1, candidate_id: 3 });
        assert!(!resp2.granted);
    }
}
