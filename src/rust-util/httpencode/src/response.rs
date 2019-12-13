use crate::{HttpBuilder, Result, Status, Version};
use bytes::BufMut;

macro_rules! status_builder {
    {
        $( $status:expr => $fn:ident; )*
    } => {
        $(
            pub fn $fn<B: BufMut>(buf: B) -> Result<HttpBuilder<B>> {
                HttpBuilder::response(
                    buf,
                    Version::Http11,
                    Status { code: $status }
                )
            }
        )*
    }
}

status_builder! {
    100 => r#continue;
    101 => switching_protocols;
    102 => processing;
    103 => early_hints;

    200 => ok;
    201 => created;
    202 => accepted;
    203 => non_authoritative_information;
    204 => no_content;
    205 => reset_content;
    206 => partial_content;
    207 => multi_status;
    208 => already_reported;
    226 => im_used;

    300 => multiple_choices;
    301 => moved_permanently;
    302 => found;
    303 => see_other;
    304 => not_modified;
    305 => use_proxy;
    // 306 is obsolete
    307 => temporary_redirect;
    308 => permanent_redirect;

    400 => bad_request;
    401 => unauthorized;
    402 => payment_required;
    403 => forbidden;
    404 => not_found;
    405 => method_not_allowed;
    406 => not_acceptable;
    407 => proxy_authentication_timeout;
    408 => request_timeout;
    409 => conflict;
    410 => gone;
    411 => length_required;
    412 => precondition_failed;
    413 => request_entity_too_large;
    414 => request_uri_too_large;
    415 => unsupported_media_type;
    416 => request_range_not_satisfiable;
    417 => expectation_failed;
    418 => im_a_teapot;
    421 => misdirected_request;
    422 => unprocessable_entity;
    423 => locked;
    424 => failed_dependency;
    425 => too_early;
    426 => upgrade_required;
    428 => precondition_required;
    429 => too_many_requests;
    451 => unavailable_for_legal_reasons;

    500 => internal_server_error;
    501 => not_implemented;
    502 => bad_gateway;
    503 => service_unavailable;
    504 => gateway_time_out;
    505 => http_version_not_supported;
    506 => variant_also_negotiates;
    507 => insufficient_storage;
    508 => loop_detected;
}
