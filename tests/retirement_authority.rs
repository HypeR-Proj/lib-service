// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[test]
fn retention_stays_with_domain_supervisors_and_out_of_shell_delegation() -> Result<(), &'static str>
{
    use hyper_os::Rights;
    use hyper_service::{process, session};
    assert!(
        session::TASK_FACTORY_CONTRACT
            .required_rights()
            .contains(Rights::RETAIN_PROCESS)
    );
    assert!(
        process::VM_MANAGER_TASK_FACTORY_CONTRACT
            .required_rights()
            .contains(Rights::RETAIN_PROCESS)
    );
    assert!(
        !process::SHELL_TASK_FACTORY_CONTRACT
            .allowed_rights()
            .contains(Rights::RETAIN_PROCESS)
    );
    let application = process::APPLICATION_STARTUP_CONTRACTS
        .iter()
        .find(|contract| contract.name() == process::TASK_FACTORY_NAME)
        .ok_or("missing standard task factory contract")?;
    assert!(
        !application
            .required_rights()
            .contains(Rights::RETAIN_PROCESS)
    );
    assert!(
        application
            .allowed_rights()
            .contains(Rights::RETAIN_PROCESS)
    );
    Ok(())
}
