#!/usr/bin/env python3
"""
Build System Auditor - Verify Makefile vs actual files
"""

import os
import re
from pathlib import Path
import subprocess

class BuildAuditor:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.makefile_path = self.base_path / 'Makefile'
        self.issues = []
        self.files_found = {}
        
    def scan_actual_files(self):
        """Scan actual files in directory"""
        print("Scanning actual files...")
        
        # Find all C files
        c_files = list(self.base_path.rglob('*.c'))
        h_files = list(self.base_path.rglob('*.h'))
        
        self.files_found = {
            'ipc': [f for f in c_files if 'ipc' in str(f)],
            'vm': [f for f in c_files if 'vm' in str(f)],
            'device': [f for f in c_files if 'device' in str(f)],
            'mach': [f for f in c_files if 'mach' in str(f) and 'device' not in str(f) and 'ipc' not in str(f)],
            'bsd': [f for f in c_files if 'bsd' in str(f)],
            'unix': [f for f in c_files if 'unix' in str(f)],
            'kernel_root': [f for f in c_files if f.parent == self.base_path / 'kernel'],
            'headers': h_files
        }
        
        for category, files in self.files_found.items():
            print(f"  {category}: {len(files)} files")
            for f in files[:3]:  # Show first 3 files
                print(f"    - {f.relative_to(self.base_path)}")
    
    def analyze_makefile(self):
        """Analyze Makefile patterns"""
        print("\nAnalyzing Makefile...")
        
        if not self.makefile_path.exists():
            self.issues.append("CRITICAL: Makefile not found")
            return
            
        with open(self.makefile_path, 'r') as f:
            makefile_content = f.read()
        
        # Extract wildcard patterns
        patterns = {
            'IPC_SRCS': r'IPC_SRCS = \$\(wildcard \$\(IPC_DIR\)/\*\.c\)',
            'VM_SRCS': r'VM_SRCS = \$\(wildcard \$\(VM_DIR\)/\*\.c\)',
            'DEVICE_SRCS': r'DEVICE_SRCS = \$\(wildcard \$\(DEVICE_DIR\)/\*\.c\)',
            'MACH_SRCS': r'MACH_SRCS = \$\(wildcard \$\(MACH_DIR\)/\*\.c\)',
            'BSD_SRCS': r'BSD_SRCS = \$\(wildcard \$\(BSD_DIR\)/\*\.c\)',
            'UNIX_SRCS': r'UNIX_SRCS = \$\(wildcard \$\(UNIX_DIR\)/\*\.c\)'
        }
        
        for var, pattern in patterns.items():
            if re.search(pattern, makefile_content):
                print(f"  ✓ {var} pattern found")
            else:
                self.issues.append(f"Missing pattern: {var}")
    
    def check_directory_structure(self):
        """Check if directories match Makefile expectations"""
        print("\nChecking directory structure...")
        
        expected_dirs = {
            'kernel/ipc': 'IPC_DIR',
            'kernel/vm': 'VM_DIR', 
            'kernel/device': 'DEVICE_DIR',
            'kernel/mach': 'MACH_DIR',
            'servers/bsd': 'BSD_DIR',
            'servers/unix': 'UNIX_DIR'
        }
        
        for dir_path, makefile_var in expected_dirs.items():
            full_path = self.base_path / dir_path
            if full_path.exists():
                c_files = list(full_path.glob('*.c'))
                print(f"  ✓ {dir_path}: {len(c_files)} .c files")
                if len(c_files) == 0 and dir_path not in ['kernel/vm', 'servers/unix']:
                    self.issues.append(f"WARNING: {dir_path} exists but has no .c files")
            else:
                print(f"  ✗ {dir_path}: MISSING")
                self.issues.append(f"CRITICAL: Missing directory {dir_path}")
    
    def check_include_paths(self):
        """Verify include paths in Makefile match actual structure"""
        print("\nChecking include paths...")
        
        expected_includes = [
            'include', 'include/mach', 'include/sys', 'include/kern', 'include/ipc'
        ]
        
        for inc_path in expected_includes:
            full_path = self.base_path / inc_path
            if full_path.exists():
                h_files = list(full_path.glob('*.h'))
                print(f"  ✓ {inc_path}: {len(h_files)} headers")
            else:
                print(f"  ✗ {inc_path}: MISSING")
                self.issues.append(f"Missing include directory: {inc_path}")
    
    def check_missing_critical_files(self):
        """Check for critical files needed for build"""
        print("\nChecking critical files...")
        
        critical_files = {
            'link.ld': 'Linker script',
            'kernel/boot.S': 'Bootstrap assembly',
            'kernel/main.c': 'Kernel entry point',
            'include/mach/mach_types.h': 'Core Mach types',
            'include/synthesis.h': 'Main synthesis header'
        }
        
        for file_path, description in critical_files.items():
            full_path = self.base_path / file_path
            if full_path.exists():
                print(f"  ✓ {file_path}: {description}")
            else:
                print(f"  ✗ {file_path}: MISSING - {description}")
                if file_path in ['link.ld', 'kernel/main.c']:
                    self.issues.append(f"CRITICAL: Missing {file_path}")
                else:
                    self.issues.append(f"WARNING: Missing {file_path}")
    
    def suggest_fixes(self):
        """Suggest fixes for identified issues"""
        print("\n" + "="*60)
        print("SUGGESTED FIXES")
        print("="*60)
        
        fixes = []
        
        # Critical fixes
        if any('link.ld' in issue for issue in self.issues):
            fixes.append({
                'priority': 1,
                'action': 'Create link.ld',
                'command': 'Create linker script with memory layout'
            })
        
        if any('kernel/main.c' in issue for issue in self.issues):
            fixes.append({
                'priority': 1, 
                'action': 'Create kernel/main.c',
                'command': 'Implement kernel_main() entry point'
            })
        
        if any('kernel/boot.S' in issue for issue in self.issues):
            fixes.append({
                'priority': 1,
                'action': 'Create kernel/boot.S', 
                'command': 'Assembly bootstrap with multiboot header'
            })
        
        # Directory fixes
        if any('kernel/vm' in issue for issue in self.issues):
            fixes.append({
                'priority': 2,
                'action': 'Populate kernel/vm',
                'command': 'mkdir -p kernel/vm && create VM stubs'
            })
            
        if any('servers/unix' in issue for issue in self.issues):
            fixes.append({
                'priority': 2,
                'action': 'Populate servers/unix',
                'command': 'mkdir -p servers/unix && create Unix server stubs'
            })
        
        # Sort by priority
        fixes.sort(key=lambda x: x['priority'])
        
        for i, fix in enumerate(fixes, 1):
            print(f"{i}. [P{fix['priority']}] {fix['action']}")
            print(f"   {fix['command']}")
        
        return fixes
    
    def run_audit(self):
        """Run complete build audit"""
        print("Starting Build System Audit...")
        print("="*60)
        
        self.scan_actual_files()
        self.analyze_makefile() 
        self.check_directory_structure()
        self.check_include_paths()
        self.check_missing_critical_files()
        
        print("\n" + "="*60)
        print("ISSUES FOUND")
        print("="*60)
        
        if not self.issues:
            print("✓ No issues found!")
        else:
            critical = [i for i in self.issues if 'CRITICAL' in i]
            warnings = [i for i in self.issues if 'WARNING' in i]
            
            if critical:
                print("CRITICAL ISSUES:")
                for issue in critical:
                    print(f"  ❌ {issue}")
            
            if warnings:
                print("\nWARNINGS:")
                for issue in warnings:
                    print(f"  ⚠️ {issue}")
        
        fixes = self.suggest_fixes()
        
        return len(self.issues) == 0, fixes

if __name__ == "__main__":
    auditor = BuildAuditor("/Users/eirikr/1_Workspace/Synthesis/merged")
    can_build, fixes = auditor.run_audit()