# scroll-revm

scroll-revm is an implementation of Scroll's EVM, utilizing the [revm](https://github.com/bluealloy/revm) library—a Rust-based Ethereum Virtual Machine (EVM) implementation. This project adopts an SDK-like pattern, allowing us to override and extend the EVM functionality with Scroll specific logic cleanly and efficiently.

## Overview

The goal of scroll-revm is to provide a flexible and maintainable solution for integrating [reth](https://github.com/paradigmxyz/reth) with the scroll-revm EVM, offering a seamless way to adapt and evolve the EVM implementation as required by Scroll's rollup environment. By leveraging revm's public API, scroll-revm makes it possible to introduce custom modifications and optimizations specific to Scroll, while maintaining compatibility with the original revm codebase.

## Features

- **Rust-based EVM:** Built on top of the revm EVM implementation, offering efficient performance and compatibility with the Ethereum ecosystem.
- **SDK-Like Structure:** Provides an easy-to-use interface for overriding and extending specific components of Scroll's EVM.
- **Modular and Maintainable:** Designed to keep the codebase clean, modular, and easy to maintain, making future updates and changes straightforward.
