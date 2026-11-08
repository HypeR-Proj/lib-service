// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn golden_vectors_match_the_c_contract() -> Result<()> {
    let request = Request::new(3, 0x0102_0304_0506_0708)?;
    let expected = [
        0x48, 0x52, 0x4e, 0x47, 1, 0, 1, 0, 0, 0, 0, 0, 3, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0,
        0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(request.encode(), expected);
    assert_eq!(Request::decode(&expected)?, request);
    let reply = Reply::success(&[0xaa, 0xbb, 0xcc])?;
    let expected = [
        0x48, 0x52, 0x4e, 0x47, 1, 0, 1, 0x80, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0xaa, 0xbb, 0xcc,
    ];
    assert_eq!(reply.as_bytes(), expected);
    assert_eq!(decode_reply(&expected, 3)?, [0xaa, 0xbb, 0xcc]);
    Ok(())
}

#[test]
fn request_rejects_every_fixed_field_and_wrong_size() -> Result<()> {
    let good = Request::new(1, u64::MAX)?.encode();
    for index in (0..12).chain(24..32) {
        let mut bad = good;
        bad[index] ^= 0x80;
        assert!(Request::decode(&bad).is_err());
    }
    for length in 0..32 {
        assert!(Request::decode(&good[..length]).is_err());
    }
    let mut extra = [0; 33];
    extra[..32].copy_from_slice(&good);
    assert!(Request::decode(&extra).is_err());
    for count in [0, 193, u32::MAX] {
        let mut bad = good;
        bad[12..16].copy_from_slice(&count.to_le_bytes());
        assert!(Request::decode(&bad).is_err());
    }
    assert!(Request::new(0, 0).is_err());
    assert!(Request::new(193, 0).is_err());
    assert_eq!(
        Request::decode(&Request::new(192, 0)?.encode())?.bytes(),
        192
    );
    Ok(())
}

#[test]
fn reply_rejects_unknown_status_wrong_lengths_and_reserved_bytes() -> Result<()> {
    let good = Reply::success(&[5; 192])?;
    for index in (0..8).chain(12..16).chain(24..32) {
        let mut bad = good.frame;
        bad[index] ^= 0x40;
        assert_eq!(decode_reply(&bad, 192), Err(Error::InvalidResponse));
    }
    for length in 0..MAX_REPLY_BYTES {
        assert_eq!(
            decode_reply(&good.as_bytes()[..length], 192),
            Err(Error::InvalidResponse)
        );
    }
    assert_eq!(
        decode_reply(good.as_bytes(), 191),
        Err(Error::InvalidResponse)
    );
    assert_eq!(
        decode_reply(good.as_bytes(), 0),
        Err(Error::InvalidResponse)
    );
    for status in [
        Status::INVALID_ARGUMENT,
        Status::NOT_SUPPORTED,
        Status::TIMED_OUT,
        Status::WOULD_BLOCK,
        Status::IO_ERROR,
    ] {
        let reply = Reply::error(status)?;
        assert_eq!(
            decode_reply(reply.as_bytes(), 1),
            Err(Error::Status(status))
        );
        let mut bad = reply.frame;
        bad[8] = 1;
        assert_eq!(decode_reply(&bad[..32], 1), Err(Error::InvalidResponse));
        assert_eq!(
            decode_reply(&reply.frame[..33], 1),
            Err(Error::InvalidResponse)
        );
    }
    let mut bad = good.frame;
    bad[16..24].copy_from_slice(&(-999i64).to_le_bytes());
    assert_eq!(decode_reply(&bad, 192), Err(Error::InvalidResponse));
    assert!(Reply::error(Status::OK).is_err());
    assert!(Reply::error(Status::ACCESS_DENIED).is_err());
    assert!(Reply::success(&[]).is_err());
    assert!(Reply::success(&[0; 193]).is_err());
    Ok(())
}

#[test]
fn startup_roles_never_grant_provider_read_to_clients() {
    use crate::random::*;
    use hyper_os::handle::Rights;
    assert_ne!(CLIENT.as_raw(), SERVER.as_raw());
    assert!(!DELEGATED_CLIENT_RIGHTS.contains(Rights::READ));
    assert_eq!(REPLY_RIGHTS, Rights::WRITE.union(Rights::WAIT));
    assert_eq!(CLIENT_CONTRACT.required_rights(), CLIENT_RIGHTS);
    assert_eq!(SERVER_CONTRACT.required_rights(), SERVER_RIGHTS);
}
