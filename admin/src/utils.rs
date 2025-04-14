use std::fmt::Display;

use itertools::Itertools;

use crate::common::Common;

pub fn link<S>(action: S, common: &Common) -> String
where
    S: AsRef<str>,
{
    format!(
        "?action={}&site={}",
        action.as_ref(),
        common.current_site_id
    )
}

pub fn link_to<S, T>(action: S, args: &[(S, T)], common: &Common) -> String
where
    S: AsRef<str>,
    T: Display,
{
    let params = args
        .iter()
        .map(|i| format!("{}={}", i.0.as_ref(), i.1))
        .join("&");
    format!(
        "?action={}&site={}&{}",
        action.as_ref(),
        common.current_site_id,
        params
    )
}
