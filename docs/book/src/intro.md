# Introduction

This project, **Mach_R**, is a from-scratch effort to build a modern, safe, and performant microkernel in pure Rust.

## Core Goal

The primary objective is to implement a microkernel that adheres to the architectural principles of the historical Mach microkernel, particularly its elegant Inter-Process Communication (IPC) model. By leveraging Rust's safety guarantees and modern language features, we aim to create a kernel that is both robust and efficient, suitable for research, education, and potentially specialized production systems.

## Project Reality

This project is an active, ongoing implementation. It is crucial to understand that this is **not** a completed operating system. The repository contains a significant amount of historical C code from projects like CMU Mach, Lites, and OSF/1. This code serves as an **architectural reference and a historical archive**, not as a basis for direct porting or integration.

The official and current development effort is focused exclusively on the pure Rust implementation located in the `synthesis/` directory.

This documentation serves as the single source of truth for the architecture, roadmap, and status of the **Mach_R** kernel.