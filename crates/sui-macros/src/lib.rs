// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn init_static_initializers(_args: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(item as syn::ItemFn);

    let body = &input.block;
    input.block = syn::parse2(quote! {
        {
            // We have some lazily-initialized static state in the program. The initializers
            // alter the thread-local hash container state any time they create a new hash
            // container. Therefore, we need to ensure that these initializers are run in a
            // separate thread before the first test thread is launched. Otherwise, they would
            // run inside of the first test thread, but not subsequent ones.
            //
            // Note that none of this has any effect on process-level determinism. Without this
            // code, we can still get the same test results from two processes started with the
            // same seed.
            //
            // However, when using sim_test(check_determinism) or MSIM_TEST_CHECK_DETERMINISM=1,
            // we want the same test invocation to be deterministic when run twice
            // _in the same process_, so we need to take care of this. This will also
            // be very important for being able to reproduce a failure that occurs in the Nth
            // iteration of a multi-iteration test run.
            std::thread::spawn(|| {
                ::telemetry_subscribers::init_for_testing();
                ::sui_framework::get_move_stdlib();
                ::sui_framework::get_sui_framework();
            }).join().unwrap();

            #body
        }
    })
    .expect("Parsing failure");

    let result = quote! {
        #input
    };

    result.into()
}

/// The sui_test macro will invoke either #[msim::test] or #[tokio::test],
/// depending on whether the simulator config var is enabled.
///
/// This should be used for tests that can meaningfully run in either environment.
#[proc_macro_attribute]
pub fn sui_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    let header = if cfg!(msim) {
        quote! {
            #[::sui_simulator::sim_test(crate = "sui_simulator", #(#args)*)]
            #[init_static_initializers]
        }
    } else {
        quote! {
            #[::tokio::test(#(#args)*)]
            // though this is not required for tokio, we do it to get logs as well.
            #[init_static_initializers]
        }
    };

    let result = quote! {
        #header
        #input
    };

    result.into()
}

/// The sim_test macro will invoke #[msim::test] if the simulator config var is enabled.
///
/// Otherwise, it will emit an ignored test - if forcibly run, the ignored test will panic.
///
/// This macro must be used in order to pass any simulator-specific arguments, such as
/// `check_determinism`, which is not understood by tokio.
#[proc_macro_attribute]
pub fn sim_test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    let result = if cfg!(msim) {
        quote! {
            #[::sui_simulator::sim_test(crate = "sui_simulator", #(#args)*)]
            #[init_static_initializers]
            #input
        }
    } else {
        let fn_name = input.sig.ident.clone();
        quote! {
            #[::core::prelude::v1::test]
            #[ignore = "simulator-only test"]
            fn #fn_name () {
                unimplemented!("this test cannot run outside the simulator");

                // paste original function to silence un-used import errors.
                #[allow(dead_code)]
                #input
            }
        }
    };

    result.into()
}
