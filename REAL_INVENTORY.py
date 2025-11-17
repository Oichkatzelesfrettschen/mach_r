#!/usr/bin/env python3
"""
REAL INVENTORY - Actually extract and catalog what we have
"""

import os
import subprocess
from pathlib import Path
import json

class RealInventory:
    def __init__(self):
        self.osfmk_path = Path("/Users/eirikr/OSFMK")
        self.extract_base = Path("/Users/eirikr/1_Workspace/Synthesis/REAL_EXTRACTED")
        self.extract_base.mkdir(exist_ok=True)
        self.inventory = {}
        
    def extract_archive(self, archive_name):
        """Extract and analyze one archive"""
        archive_path = self.osfmk_path / archive_name
        extract_dir = self.extract_base / archive_name.replace('.', '_')
        
        if not archive_path.exists():
            return None
            
        print(f"\nExtracting {archive_name}...")
        extract_dir.mkdir(exist_ok=True)
        
        # Extract based on type
        if archive_name.endswith('.tar.gz'):
            cmd = f"cd {extract_dir} && tar -xzf {archive_path} 2>/dev/null"
        elif archive_name.endswith('.tar.bz2'):
            cmd = f"cd {extract_dir} && tar -xjf {archive_path} 2>/dev/null"
        elif archive_name.endswith('.tar.Z'):
            cmd = f"cd {extract_dir} && tar -xZf {archive_path} 2>/dev/null"
        elif archive_name.endswith('.tar'):
            # Check if it's a tar of tars
            if '.tar.' in archive_name and archive_name.endswith('.tar'):
                # First extract outer tar
                cmd = f"cd {extract_dir} && tar -xf {archive_path} 2>/dev/null"
                subprocess.run(cmd, shell=True)
                # Then extract inner archives
                inner_archives = list(extract_dir.glob("*.tar*"))
                for inner in inner_archives:
                    if inner.name.endswith('.gz'):
                        cmd2 = f"cd {extract_dir} && tar -xzf {inner} 2>/dev/null"
                    elif inner.name.endswith('.Z'):
                        cmd2 = f"cd {extract_dir} && tar -xZf {inner} 2>/dev/null"
                    else:
                        cmd2 = f"cd {extract_dir} && tar -xf {inner} 2>/dev/null"
                    subprocess.run(cmd2, shell=True)
                return self.analyze_extracted(extract_dir)
            else:
                cmd = f"cd {extract_dir} && tar -xf {archive_path} 2>/dev/null"
        else:
            return None
            
        subprocess.run(cmd, shell=True)
        return self.analyze_extracted(extract_dir)
    
    def analyze_extracted(self, path):
        """Analyze extracted content"""
        stats = {
            'total_files': 0,
            'c_files': [],
            'h_files': [],
            'asm_files': [],
            'makefiles': [],
            'subsystems': {},
            'total_lines': 0
        }
        
        # Key subsystem patterns
        subsystem_patterns = {
            'vm': 'Virtual Memory',
            'mm': 'Memory Management',
            'sched': 'Scheduler',
            'kern/sched': 'Kernel Scheduler',
            'fs/': 'Filesystem',
            'net/': 'Networking',
            'ipc/': 'IPC',
            'device/': 'Device Drivers',
            'kern/': 'Kernel Core',
            'thread': 'Threading',
            'task': 'Tasks',
            'mach/': 'Mach',
            'bsd/': 'BSD Layer',
            'boot': 'Bootstrap'
        }
        
        for root, dirs, files in os.walk(path):
            for file in files:
                stats['total_files'] += 1
                full_path = Path(root) / file
                rel_path = full_path.relative_to(path)
                
                if file.endswith('.c'):
                    stats['c_files'].append(str(rel_path))
                    # Count lines
                    try:
                        with open(full_path, 'r', encoding='latin-1') as f:
                            stats['total_lines'] += len(f.readlines())
                    except:
                        pass
                        
                    # Identify subsystem
                    for pattern, name in subsystem_patterns.items():
                        if pattern in str(rel_path).lower():
                            if name not in stats['subsystems']:
                                stats['subsystems'][name] = []
                            stats['subsystems'][name].append(str(rel_path))
                            break
                            
                elif file.endswith('.h'):
                    stats['h_files'].append(str(rel_path))
                elif file.endswith(('.s', '.S')):
                    stats['asm_files'].append(str(rel_path))
                elif 'makefile' in file.lower():
                    stats['makefiles'].append(str(rel_path))
                    
        return stats
    
    def analyze_key_archives(self):
        """Analyze the most important archives"""
        key_archives = [
            'CMU-Mach-MK83.tar.bz2',  # CMU Mach microkernel
            'lites-1.1.tar.gz.tar',    # Lites Unix server
            'OSF1-1.0-src.tar.bz2',    # Full OSF/1 OS
            'mach4-i386-UK22.tar.gz',  # Mach 4 for i386
            'gnu-osfmach.tar.gz'       # GNU's OSF Mach
        ]
        
        results = {}
        
        for archive in key_archives:
            print(f"\n{'='*60}")
            print(f"ANALYZING: {archive}")
            print('='*60)
            
            stats = self.extract_archive(archive)
            if stats:
                results[archive] = stats
                
                print(f"  Total files: {stats['total_files']}")
                print(f"  C files: {len(stats['c_files'])}")
                print(f"  Headers: {len(stats['h_files'])}")
                print(f"  Assembly: {len(stats['asm_files'])}")
                print(f"  Lines of code: {stats['total_lines']:,}")
                
                if stats['subsystems']:
                    print(f"\n  SUBSYSTEMS FOUND:")
                    for subsys, files in stats['subsystems'].items():
                        print(f"    {subsys}: {len(files)} files")
                        # Show sample files
                        for f in files[:2]:
                            print(f"      - {f}")
                            
        return results
    
    def create_integration_plan(self, inventory):
        """Create a real integration plan based on what we have"""
        print("\n" + "="*80)
        print("REAL INTEGRATION PLAN - NAUTILUS STYLE")
        print("="*80)
        
        # Phase 1: Core Kernel
        print("\nPHASE 1: CORE KERNEL (Week 1)")
        print("  1. Memory Management:")
        print("     - Extract CMU-Mach-MK83 vm/* subsystem")
        print("     - Integrate vm_map.c, vm_page.c, vm_object.c")
        print("     - Add physical memory allocator")
        print("  2. Task/Thread Management:")
        print("     - Extract kern/thread.c, kern/task.c")
        print("     - Integrate scheduler from kern/sched/*")
        print("  3. IPC Foundation:")
        print("     - Actually integrate (not just copy) ipc/* files")
        print("     - Wire up mach_msg properly")
        
        # Phase 2: Device Layer
        print("\nPHASE 2: DEVICE LAYER (Week 2)")
        print("  1. Device framework from device/*")
        print("  2. Console driver (already have)")
        print("  3. Timer/clock driver")
        print("  4. Basic disk driver")
        
        # Phase 3: Server Layer
        print("\nPHASE 3: UNIX SERVER (Week 3)")
        print("  1. Extract Lites server code")
        print("  2. Implement basic syscalls")
        print("  3. Process management")
        print("  4. Simple filesystem (from Lites)")
        
        print("\nESTIMATED REAL IMPLEMENTATION:")
        print("  - 3 weeks to minimal working OS")
        print("  - 6 weeks to usable system")
        print("  - 12 weeks to stable microkernel OS")

if __name__ == "__main__":
    inventory = RealInventory()
    results = inventory.analyze_key_archives()
    
    # Save inventory
    with open('/Users/eirikr/1_Workspace/Synthesis/real_inventory.json', 'w') as f:
        json.dump(results, f, indent=2)
        
    inventory.create_integration_plan(results)
    
    print("\n" + "="*80)
    print("BOTTOM LINE:")
    print("  We have MILLIONS of lines of REAL OS code")
    print("  We integrated ZERO percent of it")
    print("  We can build a REAL OS if we actually integrate it")
    print("="*80)