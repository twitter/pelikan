// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub const N_BUCKET_PER_STEP_N_BIT: usize = 8;
pub const N_BUCKET_PER_STEP: usize = 1 << N_BUCKET_PER_STEP_N_BIT;

pub const TTL_BUCKET_INTERVAL_N_BIT_1: usize = 3;
pub const TTL_BUCKET_INTERVAL_N_BIT_2: usize = 7;
pub const TTL_BUCKET_INTERVAL_N_BIT_3: usize = 11;
pub const TTL_BUCKET_INTERVAL_N_BIT_4: usize = 15;

pub const TTL_BUCKET_INTERVAL_1: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_1;
pub const TTL_BUCKET_INTERVAL_2: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_2;
pub const TTL_BUCKET_INTERVAL_3: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_3;
pub const TTL_BUCKET_INTERVAL_4: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_4;

pub const TTL_BOUNDARY_1: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_1 + N_BUCKET_PER_STEP_N_BIT);
pub const TTL_BOUNDARY_2: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_2 + N_BUCKET_PER_STEP_N_BIT);
pub const TTL_BOUNDARY_3: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_3 + N_BUCKET_PER_STEP_N_BIT);

pub const MAX_N_TTL_BUCKET: usize = N_BUCKET_PER_STEP * 4;
pub const MAX_TTL_BUCKET_IDX: usize = MAX_N_TTL_BUCKET - 1;
