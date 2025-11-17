#!/usr/bin/env python3
"""
BRUTAL REALITY AUDIT - What the fuck do we actually have?
"""

import os
import tarfile
import subprocess
from pathlib import Path
import json

class BrutalAuditor:
    def __init__(self):
        self.base_path = Path("/Users/eirikr/1_Workspace/Synthesis")
        self.archives_path = Path("/Users/eirikr/1_Workspace/merged")
        self.inventory = {
            'archives': {},
            'actual_content': {},
            'claims': [],
            'reality': [],
            'bullshit_meter': 0
        }
        
    def extract_and_analyze_archive(self, archive_path):
        """Extract archive and analyze what's actually in it"""
        name = archive_path.name
        extract_dir = self.base_path / 'extracted' / name.replace('.', '_')
        extract_dir.mkdir(parents=True, exist_ok=True)
        
        print(f"\n{'='*60}")
        print(f"EXTRACTING: {name}")
        print('='*60)
        
        try:
            # Determine extraction method
            if name.endswith('.tar.gz') or name.endswith('.tgz'):
                cmd = f"tar -xzf {archive_path} -C {extract_dir} 2>/dev/null"
            elif name.endswith('.tar.bz2'):
                cmd = f"tar -xjf {archive_path} -C {extract_dir} 2>/dev/null"
            elif name.endswith('.tar.Z'):
                cmd = f"tar -xZf {archive_path} -C {extract_dir} 2>/dev/null"
            elif name.endswith('.tar'):
                cmd = f"tar -xf {archive_path} -C {extract_dir} 2>/dev/null"
            else:
                return None
                
            subprocess.run(cmd, shell=True)
            
            # Analyze content
            stats = self.analyze_directory(extract_dir)
            self.inventory['archives'][name] = stats
            
            # Print immediate findings
            print(f"  C files: {stats['c_files']}")
            print(f"  Headers: {stats['h_files']}")
            print(f"  Assembly: {stats['asm_files']}")
            print(f"  Total lines: {stats['total_lines']:,}")
            
            # Look for key subsystems
            subsystems = self.identify_subsystems(extract_dir)
            if subsystems:
                print(f"  SUBSYSTEMS FOUND:")
                for sys in subsystems:
                    print(f"    - {sys}")
            
            return stats
            
        except Exception as e:
            print(f"  ERROR: {e}")
            return None
    
    def analyze_directory(self, dir_path):
        """Get real statistics about a directory"""
        stats = {
            'c_files': 0,
            'h_files': 0,
            'asm_files': 0,
            'makefiles': 0,
            'total_lines': 0,
            'subsystems': []
        }
        
        for root, dirs, files in os.walk(dir_path):
            for file in files:
                if file.endswith('.c'):
                    stats['c_files'] += 1
                    # Count lines
                    try:
                        with open(os.path.join(root, file), 'r', encoding='latin-1') as f:
                            stats['total_lines'] += len(f.readlines())
                    except:
                        pass
                elif file.endswith('.h'):
                    stats['h_files'] += 1
                elif file.endswith(('.s', '.S', '.asm')):
                    stats['asm_files'] += 1
                elif 'makefile' in file.lower():
                    stats['makefiles'] += 1
                    
        return stats
    
    def identify_subsystems(self, dir_path):
        """Identify actual OS subsystems present"""
        subsystems = []
        
        # Key directories that indicate subsystems
        indicators = {
            'vm': 'Virtual Memory',
            'mm': 'Memory Management',
            'sched': 'Scheduler',
            'fs': 'Filesystem',
            'net': 'Networking',
            'ipc': 'IPC/Message Passing',
            'device': 'Device Drivers',
            'kern': 'Kernel Core',
            'thread': 'Threading',
            'task': 'Task Management',
            'port': 'Port/Capability System',
            'mach': 'Mach Microkernel',
            'bsd': 'BSD Compatibility',
            'posix': 'POSIX Compliance',
            'boot': 'Bootstrap/Loader'
        }
        
        for root, dirs, files in os.walk(dir_path):
            path_lower = root.lower()
            for indicator, name in indicators.items():
                if indicator in path_lower and name not in subsystems:
                    # Verify it has actual code
                    c_files = [f for f in files if f.endswith('.c')]
                    if c_files:
                        subsystems.append(name)
                        
        return subsystems
    
    def audit_current_kernel(self):
        """Audit what we actually built"""
        print("\n" + "="*60)
        print("CURRENT 'SYNTHESIS' KERNEL AUDIT")
        print("="*60)
        
        merged = self.base_path / 'merged'
        
        # Count what we have
        actual_files = {
            'main.c': False,
            'boot.S': False,
            'compat.c': False,
            'ipc_implementations': 0,
            'device_drivers': 0,
            'memory_management': 0,
            'filesystem': 0,
            'scheduler': 0,
            'networking': 0
        }
        
        # Check what exists
        if (merged / 'kernel' / 'main.c').exists():
            actual_files['main.c'] = True
            with open(merged / 'kernel' / 'main.c', 'r') as f:
                lines = len(f.readlines())
                print(f"  main.c: {lines} lines (mostly printf)")
                
        if (merged / 'kernel' / 'boot.S').exists():
            actual_files['boot.S'] = True
            print(f"  boot.S: Basic multiboot stub")
            
        if (merged / 'kernel' / 'compat.c').exists():
            actual_files['compat.c'] = True
            print(f"  compat.c: Empty stub functions")
            
        # Count IPC files we copied
        ipc_dir = merged / 'kernel' / 'ipc'
        if ipc_dir.exists():
            actual_files['ipc_implementations'] = len(list(ipc_dir.glob('*.c')))
            print(f"  IPC: {actual_files['ipc_implementations']} files (UNINTEGRATED COPIES)")
            
        print("\nREALITY CHECK:")
        print("  ✗ NO working memory management")
        print("  ✗ NO scheduler")
        print("  ✗ NO filesystem")
        print("  ✗ NO real device drivers")
        print("  ✗ NO networking")
        print("  ✗ NO user mode")
        print("  ✗ NO system calls")
        print("  ✓ Can print to VGA console")
        print("  ✓ Can halt")
        
        return actual_files
    
    def compare_claims_to_reality(self):
        """Compare what we claimed vs what we have"""
        print("\n" + "="*60)
        print("FALSIFIABLE CLAIMS vs REALITY")
        print("="*60)
        
        claims = [
            ("Synthesized 4 major OS implementations", False, "Just copied random files"),
            ("Mach microkernel IPC", False, "Copied files, not integrated"),
            ("BSD compatibility layer", False, "Copied network stack, doesn't work"),
            ("1.3M lines integrated", False, "27K lines copied, ~200 lines written"),
            ("Unified syscall interface", False, "Empty stub functions"),
            ("Modern toolchain support", True, "Cross-compiler works"),
            ("Bootable kernel", True, "Boots to 'Hello World'"),
            ("Operating System", False, "It's a bootloader with printf")
        ]
        
        true_claims = 0
        for claim, is_true, reality in claims:
            symbol = "✓" if is_true else "✗"
            print(f"{symbol} CLAIM: {claim}")
            print(f"  REALITY: {reality}")
            if is_true:
                true_claims += 1
                
        bullshit_percentage = ((len(claims) - true_claims) / len(claims)) * 100
        print(f"\nBULLSHIT METER: {bullshit_percentage:.0f}%")
        
        return bullshit_percentage
    
    def run_complete_audit(self):
        """Run the complete brutal audit"""
        print("BRUTAL REALITY AUDIT - SYNTHESIS OS")
        print("="*80)
        
        # First, analyze all archives
        archive_dir = Path("/Users/eirikr/1_Workspace/merged")
        archives = list(archive_dir.glob("*.tar*")) + list(archive_dir.glob("*.tgz"))
        
        print(f"\nFound {len(archives)} archives to analyze")
        
        total_lines = 0
        total_c_files = 0
        all_subsystems = set()
        
        for archive in archives[:5]:  # Analyze first 5 for speed
            stats = self.extract_and_analyze_archive(archive)
            if stats:
                total_lines += stats['total_lines']
                total_c_files += stats['c_files']
                all_subsystems.update(stats.get('subsystems', []))
        
        print("\n" + "="*60)
        print("TOTAL AVAILABLE RESOURCES")
        print("="*60)
        print(f"  Total C files available: {total_c_files}")
        print(f"  Total lines available: {total_lines:,}")
        print(f"  Subsystems available: {len(all_subsystems)}")
        
        # Audit current kernel
        self.audit_current_kernel()
        
        # Compare claims
        bullshit = self.compare_claims_to_reality()
        
        # Final verdict
        print("\n" + "="*80)
        print("FINAL VERDICT")
        print("="*80)
        print("What we have: A 'Hello World' bootloader masquerading as an OS")
        print("What we claimed: A synthesized operating system")
        print("What we could build: With proper integration, an actual microkernel")
        print(f"Current completion: ~0.5% of a real OS")
        print(f"Bullshit level: {bullshit:.0f}%")
        
        # Save results
        with open(self.base_path / 'brutal_audit.json', 'w') as f:
            json.dump(self.inventory, f, indent=2)

if __name__ == "__main__":
    auditor = BrutalAuditor()
    auditor.run_complete_audit()