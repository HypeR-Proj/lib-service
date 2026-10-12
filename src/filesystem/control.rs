// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Small control plane wire format. Filesystem data never travels here.

extern crate alloc;

pub const SYSTEM_FSTAB: &str = "/system/etc/fstab";

pub const MAX_BYTES: usize = 8192;
#[derive(Debug, Eq, PartialEq)]
pub enum Request<'a> {
    List,
    All {
        table: &'a str,
    },
    Mount {
        source: &'a str,
        target: &'a str,
        format: &'a str,
        options: &'a str,
    },
    Unmount {
        target: &'a str,
    },
    Entry {
        table: &'a str,
        name: &'a str,
    },
}

/// NUL-separated UTF-8 fields keep fstab paths containing whitespace unambiguous.
pub fn decode(bytes: &[u8]) -> Option<Request<'_>> {
    if bytes.len() > MAX_BYTES || bytes.last() != Some(&0) {
        return None;
    }
    let mut fields = bytes[..bytes.len() - 1].split(|byte| *byte == 0);
    let mut next = || core::str::from_utf8(fields.next()?).ok();
    let request = match next()? {
        "list" => Request::List,
        "all" => Request::All { table: next()? },
        "mount" => Request::Mount {
            source: next()?,
            target: next()?,
            format: next()?,
            options: next()?,
        },
        "umount" => Request::Unmount { target: next()? },
        "entry" => Request::Entry {
            table: next()?,
            name: next()?,
        },
        _ => return None,
    };
    fields.next().is_none().then_some(request)
}

/// Exchange one command through a private response pipe. Shared connector
/// capabilities cannot steal another process's response or queue file data.
pub fn execute(
    startup: &mut hyper_os::startup::Startup<'_>,
    bytes: &[u8],
) -> hyper_os::Result<(bool, alloc::string::String)> {
    let connector = hyper_os::capability_channel::CapabilityChannel::from_handle(
        startup.take(super::CONTROL_CLIENT)?,
    );
    execute_channel(&connector, bytes)
}

/// Reuse a supervisor's connector without consuming its delegation authority.
pub fn execute_channel(
    connector: &hyper_os::capability_channel::CapabilityChannel,
    bytes: &[u8],
) -> hyper_os::Result<(bool, alloc::string::String)> {
    use hyper_os::{
        capability_channel::CapabilityDisposition,
        handle::{ByteChannelObject, Rights, RightsOffer},
        wait::{ObjectSignals, WaitItem, wait_many},
    };
    if decode(bytes).is_none() {
        return Err(hyper_os::Error::InvalidResponse);
    }
    let (client, server) = hyper_os::channel::create_pair()?;
    client.as_byte_channel().send(bytes)?;
    let deadline = hyper_os::time::deadline_after(core::time::Duration::from_secs(120))?.as_raw();
    let mut server = Some(server);
    crate::io::send_capabilities(
        connector,
        super::CONTROL_CONNECT,
        &mut [CapabilityDisposition::move_handle(
            &mut server,
            RightsOffer::Exact(Rights::READ.union(Rights::WRITE).union(Rights::WAIT)),
        )?],
        deadline,
    )?;
    wait_many(
        &[WaitItem::new(
            client.as_handle_ref(),
            ObjectSignals::<ByteChannelObject>::READABLE
                .union(ObjectSignals::<ByteChannelObject>::PEER_CLOSED),
        )],
        deadline,
    )?;
    let mut response = alloc::vec![0; MAX_BYTES];
    let length = client.as_byte_channel().try_receive(&mut response)?;
    let text =
        core::str::from_utf8(&response[..length]).map_err(|_| hyper_os::Error::InvalidResponse)?;
    if let Some(text) = text.strip_prefix("OK\n") {
        Ok((true, text.into()))
    } else if let Some(text) = text.strip_prefix("ERROR\n") {
        Ok((false, text.into()))
    } else {
        Err(hyper_os::Error::InvalidResponse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn caller_selects_the_mount_table() {
        assert_eq!(
            decode(b"all\0/system/etc/fstab\0"),
            Some(Request::All {
                table: "/system/etc/fstab"
            })
        );
        assert_eq!(
            decode(b"entry\0/boot/etc/fstab\0/media/my disk\0"),
            Some(Request::Entry {
                table: "/boot/etc/fstab",
                name: "/media/my disk"
            })
        );
    }

    #[test]
    fn strict_fields_preserve_whitespace() {
        assert_eq!(
            decode(b"umount\0/media/my disk\0"),
            Some(Request::Unmount {
                target: "/media/my disk"
            })
        );
        for bad in [
            b"list".as_slice(),
            b"list\0extra\0",
            b"mount\0config\0",
            b"all\0",
            b"all\0/table\0extra\0",
            b"entry\0/table\0",
        ] {
            assert!(decode(bad).is_none());
        }
    }
}
