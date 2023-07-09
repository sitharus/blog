use core::fmt;

pub enum AdminMenuPages {
    Dashboard,
    Account,
    Posts,
    NewPost,
    Settings,
    Links,
    Comments,
    Pages,
    Media,
    Fediverse,
}
impl fmt::Display for AdminMenuPages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AdminMenuPages::Dashboard => write!(f, "dashboard"),
            AdminMenuPages::Account => write!(f, "account"),
            AdminMenuPages::Posts => write!(f, "posts"),
            AdminMenuPages::NewPost => write!(f, "newpost"),
            AdminMenuPages::Settings => write!(f, "settings"),
            AdminMenuPages::Links => write!(f, "links"),
            AdminMenuPages::Comments => write!(f, "comments"),
            AdminMenuPages::Pages => write!(f, "pages"),
            AdminMenuPages::Media => write!(f, "media"),
            AdminMenuPages::Fediverse => write!(f, "fediverse"),
        }
    }
}

impl PartialEq<&str> for AdminMenuPages {
    fn eq(&self, rhs: &&str) -> bool {
        let str_value = self.to_string();
        return str_value == *rhs;
    }
}
