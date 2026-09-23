//! Windows ACL protection for local recovery backups.
//!
//! Recovery files may contain the original MCP configuration, including
//! credentials. The write path fails closed when Windows cannot apply a
//! protected DACL containing only SYSTEM and the current owner.

use std::path::Path;

#[cfg(windows)]
fn current_sid() -> Result<String, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TokenUser, TOKEN_QUERY,
            TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err("Cannot determine backup owner".into());
        }
        let result = (|| {
            let mut size = 0;
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut size);
            // usize storage provides alignment for TOKEN_USER and its embedded SID.
            let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            ) == 0
            {
                return Err("Cannot read backup owner token".into());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut text = std::ptr::null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut text) == 0 {
                return Err("Cannot resolve backup owner SID".into());
            }
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            Ok(sid)
        })();
        CloseHandle(token);
        result
    }
}

#[cfg(not(windows))]
pub fn secure_backup_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn secure_backup_directory(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
    use windows_sys::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        PSECURITY_DESCRIPTOR,
    };

    super::validate_path(path)?;
    // Explicit current token SID, not OWNER RIGHTS: existing directory owners
    // need not be the current user. Children inherit only these two entries.
    let sid = current_sid()?;
    let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})\0")
        .encode_utf16()
        .collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
    let mut descriptor_size = 0u32;
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor,
            &mut descriptor_size,
        )
    };
    if converted == 0 || descriptor.is_null() {
        return Err("Unable to create protected backup ACL".into());
    }

    let result = (|| {
        let mut present = 0;
        let mut defaulted = 0;
        let mut dacl = std::ptr::null_mut();
        let ok = unsafe {
            GetSecurityDescriptorDacl(descriptor, &mut present, &mut dacl, &mut defaulted)
        };
        if ok == 0 || present == 0 || dacl.is_null() {
            return Err("Unable to inspect protected backup ACL".to_string());
        }
        let name: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        let status = unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                dacl,
                std::ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!(
                "Unable to protect backup ACL (Windows error {status})"
            ));
        }
        verify_private_acl(path, true)
    })();
    unsafe {
        LocalFree(descriptor as _);
    }
    result
}

#[cfg(windows)]
fn verify_private_acl(path: &Path, protected: bool) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::{Authorization::*, *},
    };
    let name: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut descriptor = std::ptr::null_mut();
    let mut acl = std::ptr::null_mut();
    unsafe {
        let result = GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut acl,
            std::ptr::null_mut(),
            &mut descriptor,
        );
        if result != 0 {
            return Err(format!("Cannot verify backup ACL: {result}"));
        }
        let checked = (|| {
            let mut control = 0;
            let mut revision = 0;
            if GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) == 0
                || (protected && control & SE_DACL_PROTECTED == 0)
                || acl.is_null()
                || (*acl).AceCount != 2
            {
                return Err("Backup DACL is not private".into());
            }
            let current = current_sid()?;
            let mut principals = Vec::new();
            for i in 0..2 {
                let mut ace = std::ptr::null_mut();
                if GetAce(acl, i, &mut ace) == 0 {
                    return Err("Cannot inspect backup ACE".into());
                }
                let allow = &*ace.cast::<ACCESS_ALLOWED_ACE>();
                if allow.Header.AceType != 0
                    || allow.Mask != 0x1f01ff
                    || u32::from(allow.Header.AceFlags) & INHERIT_ONLY_ACE != 0
                {
                    return Err("Unexpected backup permissions".into());
                }
                let mut sid_text = std::ptr::null_mut();
                if ConvertSidToStringSidW(
                    (&allow.SidStart as *const u32).cast_mut().cast(),
                    &mut sid_text,
                ) == 0
                {
                    return Err("Cannot inspect backup SID".into());
                }
                let mut len = 0;
                while *sid_text.add(len) != 0 {
                    len += 1;
                }
                principals.push(String::from_utf16_lossy(std::slice::from_raw_parts(
                    sid_text, len,
                )));
                LocalFree(sid_text.cast());
            }
            principals.sort();
            let mut expected = vec![current, "S-1-5-18".into()];
            expected.sort();
            if principals != expected {
                return Err("Backup ACL grants unexpected principals".into());
            }
            Ok(())
        })();
        LocalFree(descriptor);
        checked
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn protects_a_temporary_backup_directory() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("backup");
        fs::create_dir(&path).unwrap();
        secure_backup_directory(&path).unwrap();
        fs::write(path.join("secret.json"), br#"{"token":"fixture"}"#).unwrap();
        verify_private_acl(&path, true).unwrap();
        verify_private_acl(&path.join("secret.json"), false).unwrap();
    }
}
