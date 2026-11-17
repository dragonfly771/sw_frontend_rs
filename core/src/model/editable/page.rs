//! Big encoder setting (Setup > Display). The page editables of the former
//! Setup > Pages menu are gone: the pages are set up on the page itself
//! (Page Layout, controller/page_layout.rs).

use super::{Content, EditableFuncs, EnumParams, Params};
use crate::{
    persist,
    utils::{TString, Variant},
    CoreController, CoreModel, Echo,
};

const ENC_PAGES: &str = "Pages";
const ENC_MC: &str = "MC";

/// what the big encoder does on a page: switch pages (new) or adjust MC
pub struct BigEncoder;
impl EditableFuncs for BigEncoder {
    fn name() -> &'static str {
        "Big Encoder"
    }

    fn content(cm: &mut CoreModel, _cc: &mut CoreController) -> Content {
        let s = if cm.config.big_encoder_pages {
            ENC_PAGES
        } else {
            ENC_MC
        };
        Content::Enum(TString::<16>::from_str(s))
    }

    fn params(_cm: &CoreModel) -> Params {
        Params::Enum(EnumParams {
            variants: [ENC_PAGES, ENC_MC, "", "", ""],
        })
    }

    fn set_content(cm: &mut CoreModel, cc: &mut CoreController, content: Content) {
        if let Content::Enum(val) = content {
            persist::persist_set(
                cc,
                cm,
                Variant::Bool(val.as_str() == ENC_PAGES),
                crate::PersistenceId::BigEncoder,
                Echo::None,
            );
        }
    }
}
