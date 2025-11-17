#!/usr/bin/env python3
"""
OS Synthesis Analysis Tool
Analyzes and compares multiple OS codebases for synthesis
"""

import os
import subprocess
import json
import sys
from pathlib import Path
from collections import defaultdict
import re

class OSAnalyzer:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.systems = {
            'CMU-Mach-MK83': 'CMU-Mach-MK83',
            'OSF1': 'OSF1-base/osf1src',
            'Lites': 'Lites-1.1/lites-1.1',
            'Mach4-i386': 'mach4-i386/mach4-i386',
            'GNU-OSFMach': 'gnu-osfmach/gnu-osfmach'
        }
        self.results = defaultdict(dict)
    
    def run_ctags(self, system_name):
        """Generate ctags for a system"""
        path = self.base_path / self.systems[system_name]
        print(f"Generating ctags for {system_name}...")
        
        cmd = f"ctags -R --c-kinds=+p --fields=+S --extras=+q -f {path}/tags {path}"
        subprocess.run(cmd, shell=True, capture_output=True)
        
        # Count symbols
        if (path / 'tags').exists():
            with open(path / 'tags', 'r', encoding='latin-1') as f:
                lines = f.readlines()
                self.results[system_name]['total_symbols'] = len(lines)
                
                # Categorize symbols
                functions = [l for l in lines if '\tf\t' in l or '\tp\t' in l]
                structs = [l for l in lines if '\ts\t' in l]
                macros = [l for l in lines if '\td\t' in l]
                
                self.results[system_name]['functions'] = len(functions)
                self.results[system_name]['structs'] = len(structs)
                self.results[system_name]['macros'] = len(macros)
    
    def analyze_complexity(self, system_name):
        """Run lizard complexity analysis"""
        path = self.base_path / self.systems[system_name]
        print(f"Analyzing complexity for {system_name}...")
        
        cmd = f"python3 -m lizard {path} --CCN 10 --length 100 --csv"
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        
        if result.stdout:
            lines = result.stdout.strip().split('\n')[1:]  # Skip header
            total_complexity = 0
            high_complexity_funcs = 0
            
            for line in lines:
                if line:
                    parts = line.split(',')
                    if len(parts) > 1:
                        try:
                            ccn = int(parts[1])
                            total_complexity += ccn
                            if ccn > 10:
                                high_complexity_funcs += 1
                        except:
                            pass
            
            self.results[system_name]['total_complexity'] = total_complexity
            self.results[system_name]['high_complexity_functions'] = high_complexity_funcs
    
    def count_lines(self, system_name):
        """Count lines of code"""
        path = self.base_path / self.systems[system_name]
        print(f"Counting lines for {system_name}...")
        
        cmd = f"find {path} -name '*.c' -o -name '*.h' | xargs wc -l | tail -1"
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        
        if result.stdout:
            try:
                total = int(result.stdout.split()[0])
                self.results[system_name]['total_lines'] = total
            except:
                self.results[system_name]['total_lines'] = 0
    
    def find_common_files(self):
        """Find files that exist in multiple systems"""
        file_map = defaultdict(list)
        
        for system_name in self.systems:
            path = self.base_path / self.systems[system_name]
            for root, dirs, files in os.walk(path):
                for file in files:
                    if file.endswith(('.c', '.h')):
                        relative = os.path.relpath(os.path.join(root, file), path)
                        file_map[file].append((system_name, relative))
        
        # Find overlaps
        overlaps = {k: v for k, v in file_map.items() if len(v) > 1}
        return overlaps
    
    def analyze_ipc_mechanisms(self):
        """Analyze IPC mechanisms in each system"""
        ipc_patterns = {
            'mach_msg': r'mach_msg\s*\(',
            'port_allocate': r'port_allocate\s*\(',
            'task_create': r'task_create\s*\(',
            'thread_create': r'thread_create\s*\(',
            'vm_allocate': r'vm_allocate\s*\(',
            'semaphore': r'semaphore_\w+\s*\(',
            'mutex': r'mutex_\w+\s*\(',
            'condition': r'condition_\w+\s*\('
        }
        
        for system_name in self.systems:
            path = self.base_path / self.systems[system_name]
            print(f"Analyzing IPC for {system_name}...")
            
            ipc_counts = defaultdict(int)
            
            for pattern_name, pattern in ipc_patterns.items():
                cmd = f"grep -r '{pattern}' {path} --include='*.c' --include='*.h' | wc -l"
                result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
                try:
                    count = int(result.stdout.strip())
                    if count > 0:
                        ipc_counts[pattern_name] = count
                except:
                    pass
            
            self.results[system_name]['ipc_mechanisms'] = dict(ipc_counts)
    
    def generate_report(self):
        """Generate analysis report"""
        print("\n" + "="*80)
        print("OS SYNTHESIS ANALYSIS REPORT")
        print("="*80)
        
        for system in self.systems:
            print(f"\n{system}:")
            print("-" * 40)
            
            if system in self.results:
                for key, value in self.results[system].items():
                    if isinstance(value, dict):
                        print(f"  {key}:")
                        for k, v in value.items():
                            print(f"    - {k}: {v}")
                    else:
                        print(f"  {key}: {value:,}" if isinstance(value, int) else f"  {key}: {value}")
        
        # Find overlaps
        print("\n" + "="*80)
        print("FILE OVERLAPS")
        print("="*80)
        overlaps = self.find_common_files()
        
        for filename, systems in list(overlaps.items())[:20]:  # Show first 20
            print(f"\n{filename}:")
            for system, path in systems:
                print(f"  - {system}: {path}")
        
        # Calculate synthesis complexity
        print("\n" + "="*80)
        print("SYNTHESIS METRICS")
        print("="*80)
        
        total_lines = sum(r.get('total_lines', 0) for r in self.results.values())
        total_symbols = sum(r.get('total_symbols', 0) for r in self.results.values())
        total_complexity = sum(r.get('total_complexity', 0) for r in self.results.values())
        
        print(f"Total lines across all systems: {total_lines:,}")
        print(f"Total symbols: {total_symbols:,}")
        print(f"Total complexity: {total_complexity:,}")
        print(f"File overlaps: {len(overlaps)}")
        
        # Estimate merge difficulty
        merge_difficulty = (len(overlaps) * 10) + (total_complexity / 100)
        print(f"\nEstimated Merge Difficulty Score: {merge_difficulty:.2f}")
        
        if merge_difficulty < 500:
            print("  -> Manageable synthesis")
        elif merge_difficulty < 1500:
            print("  -> Moderate difficulty synthesis")
        else:
            print("  -> High difficulty synthesis - consider phased approach")
    
    def run_full_analysis(self):
        """Run complete analysis pipeline"""
        for system in self.systems:
            print(f"\nAnalyzing {system}...")
            self.run_ctags(system)
            self.analyze_complexity(system)
            self.count_lines(system)
        
        self.analyze_ipc_mechanisms()
        self.generate_report()
        
        # Save results to JSON
        with open(self.base_path / 'analysis_results.json', 'w') as f:
            json.dump(self.results, f, indent=2)
        
        print(f"\nResults saved to {self.base_path / 'analysis_results.json'}")

if __name__ == "__main__":
    analyzer = OSAnalyzer("/Users/eirikr/1_Workspace/Synthesis")
    analyzer.run_full_analysis()