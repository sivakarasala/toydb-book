/// Chapter 14: Raft Leader Election — SOLUTION

#[derive(Debug, Clone, PartialEq)]
pub enum NodeState { Follower, Candidate, Leader }

pub struct RaftNode {
    pub id: u64, pub state: NodeState, pub current_term: u64,
    pub voted_for: Option<u64>, pub votes_received: usize, pub peer_count: usize,
}

#[derive(Debug)]
pub struct RequestVote { pub term: u64, pub candidate_id: u64 }
#[derive(Debug)]
pub struct VoteResponse { pub term: u64, pub granted: bool }

impl RaftNode {
    pub fn new(id: u64, peer_count: usize) -> Self {
        RaftNode { id, state: NodeState::Follower, current_term: 0, voted_for: None, votes_received: 0, peer_count }
    }

    pub fn start_election(&mut self) {
        self.current_term += 1;
        self.state = NodeState::Candidate;
        self.voted_for = Some(self.id);
        self.votes_received = 1;
    }

    pub fn handle_vote_request(&mut self, req: &RequestVote) -> VoteResponse {
        if req.term > self.current_term {
            self.current_term = req.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
        }
        let granted = req.term >= self.current_term
            && (self.voted_for.is_none() || self.voted_for == Some(req.candidate_id));
        if granted { self.voted_for = Some(req.candidate_id); }
        VoteResponse { term: self.current_term, granted }
    }

    pub fn handle_vote_response(&mut self, resp: &VoteResponse) {
        if resp.term > self.current_term {
            self.current_term = resp.term;
            self.state = NodeState::Follower;
            self.voted_for = None;
            return;
        }
        if resp.granted && self.state == NodeState::Candidate {
            self.votes_received += 1;
            if self.votes_received >= self.majority() {
                self.state = NodeState::Leader;
            }
        }
    }

    fn majority(&self) -> usize { (self.peer_count + 1) / 2 + 1 }
}

fn main() {
    println!("=== Chapter 14: Raft Election — Solution ===");
    let mut node = RaftNode::new(1, 4);
    println!("Initial: {:?}", node.state);
    node.start_election();
    println!("After election start: {:?} (term {})", node.state, node.current_term);
    node.handle_vote_response(&VoteResponse { term: 1, granted: true });
    node.handle_vote_response(&VoteResponse { term: 1, granted: true });
    println!("After 2 votes: {:?}", node.state);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn test_initial() { let n = RaftNode::new(1, 4); assert_eq!(n.state, NodeState::Follower); }
    #[test] fn test_election() { let mut n = RaftNode::new(1, 4); n.start_election(); assert_eq!(n.state, NodeState::Candidate); assert_eq!(n.current_term, 1); }
    #[test] fn test_win() {
        let mut n = RaftNode::new(1, 4); n.start_election();
        n.handle_vote_response(&VoteResponse { term: 1, granted: true });
        assert_eq!(n.state, NodeState::Candidate);
        n.handle_vote_response(&VoteResponse { term: 1, granted: true });
        assert_eq!(n.state, NodeState::Leader);
    }
    #[test] fn test_step_down() {
        let mut n = RaftNode::new(1, 4); n.start_election();
        n.handle_vote_response(&VoteResponse { term: 5, granted: false });
        assert_eq!(n.state, NodeState::Follower); assert_eq!(n.current_term, 5);
    }
    #[test] fn test_vote_grant() {
        let mut n = RaftNode::new(2, 4);
        assert!(n.handle_vote_request(&RequestVote { term: 1, candidate_id: 1 }).granted);
        assert!(!n.handle_vote_request(&RequestVote { term: 1, candidate_id: 3 }).granted);
    }
}
