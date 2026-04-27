export interface StakingPosition {
  userAddress: string;
  stakedAmount: string;
  lockupDays: number;
  lockupEndTime: string;
  votingPower: string;
  delegatedTo?: string;
  delegatedFrom: string[];
  earnedRewards: string;
  createdAt: string;
  updatedAt: string;
}

export interface StakeRequest {
  userAddress: string;
  amount: string;
  lockupDays?: number;
}

export interface UnstakeRequest {
  userAddress: string;
  amount: string;
}

export interface DelegateRequest {
  userAddress: string;
  delegateTo: string;
}

export interface RevokeDelegationRequest {
  userAddress: string;
}

export interface ClaimRewardsRequest {
  userAddress: string;
}

export interface StakingRewardConfig {
  baseAprBps: number;
  lockupBonusBps: number;
  epochDurationSeconds: number;
}
