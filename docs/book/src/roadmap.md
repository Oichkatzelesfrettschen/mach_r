# Engineering Roadmap

The development of Mach_R follows a realistic, phased approach. Each phase builds upon the last, delivering a concrete, testable set of features. This ensures that we have a working, demonstrable kernel at every stage of development.

The roadmap is divided into the following major phases. See the subsections for detailed goals and deliverables for each phase.

- **Phase 0: Foundation**: Setting up the development environment, tools, and foundational code structure.

- **Phase 1: Core IPC**: Implementing the absolute core of the microkernel—the port-based IPC system.

- **Phase 2: Memory Management**: Building the virtual memory subsystem, including address spaces and the external pager interface.

- **Phase 3: Task & Thread Management**: Implementing the task and thread abstractions and a preemptive scheduler.

- **Phase 4: Bootstrap & Hardware**: Getting the kernel to boot on real (virtualized) hardware and interact with basic devices.

- **Phase 5: System Servers**: Developing the first user-space servers that provide higher-level OS functionality (e.g., a simple file server).