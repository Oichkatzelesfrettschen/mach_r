#!/usr/bin/env python3
"""
IPC Interface Mapper and Synthesis Tool
Maps and compares IPC mechanisms across different OS implementations
"""

import os
import re
import subprocess
from pathlib import Path
from collections import defaultdict, Counter
import json
import networkx as nx
import matplotlib.pyplot as plt

class IPCMapper:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.systems = {
            'CMU-Mach-MK83': self.base_path / 'CMU-Mach-MK83',
            'Lites': self.base_path / 'Lites-1.1/lites-1.1',
            'Mach4-i386': self.base_path / 'mach4-i386/mach4-i386',
            'GNU-OSFMach': self.base_path / 'gnu-osfmach/gnu-osfmach'
        }
        self.ipc_patterns = {}
        self.call_graphs = {}
        self.interfaces = defaultdict(dict)
        
    def extract_mach_messages(self, system_name):
        """Extract Mach message definitions"""
        path = self.systems[system_name]
        msg_types = defaultdict(list)
        
        # Pattern for Mach message structures
        patterns = {
            'msg_type': r'typedef\s+struct\s+(\w*msg\w*)',
            'msg_send': r'mach_msg_send\s*\([^)]*\)',
            'msg_recv': r'mach_msg_receive\s*\([^)]*\)',
            'port_ops': r'(mach_port_\w+)\s*\(',
            'msg_headers': r'mach_msg_header_t\s+(\w+)',
            'msg_id': r'#define\s+(\w+_MSG_ID)\s+(\d+)'
        }
        
        for pattern_name, pattern in patterns.items():
            cmd = f"grep -r '{pattern}' {path} --include='*.h' --include='*.c' 2>/dev/null"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            
            if result.stdout:
                for line in result.stdout.strip().split('\n'):
                    if ':' in line:
                        file_path, match = line.split(':', 1)
                        matches = re.findall(pattern, match)
                        if matches:
                            msg_types[pattern_name].extend(matches)
        
        self.interfaces[system_name]['mach_messages'] = dict(msg_types)
        return msg_types
    
    def extract_function_signatures(self, system_name):
        """Extract IPC-related function signatures"""
        path = self.systems[system_name]
        signatures = defaultdict(list)
        
        # Key IPC functions to find
        ipc_functions = [
            'mach_msg', 'mach_msg_send', 'mach_msg_receive',
            'port_allocate', 'port_deallocate', 'port_insert_right',
            'task_create', 'task_terminate', 'task_suspend',
            'thread_create', 'thread_terminate', 'thread_suspend',
            'vm_allocate', 'vm_deallocate', 'vm_map',
            'semaphore_create', 'semaphore_signal', 'semaphore_wait',
            'mutex_lock', 'mutex_unlock', 'mutex_init'
        ]
        
        for func in ipc_functions:
            # Look for function definitions
            pattern = f'{func}\\s*\\([^{{]*'
            cmd = f"grep -A2 '{pattern}' {path}/**/*.c {path}/**/*.h 2>/dev/null | head -20"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            
            if result.stdout:
                # Clean up and extract signature
                sig = self.clean_signature(result.stdout)
                if sig:
                    signatures[func] = sig
        
        self.interfaces[system_name]['function_signatures'] = dict(signatures)
        return signatures
    
    def clean_signature(self, raw_sig):
        """Clean up function signature"""
        # Remove file paths and clean up
        lines = raw_sig.split('\n')
        cleaned = []
        for line in lines:
            if ':' in line:
                _, content = line.split(':', 1)
                cleaned.append(content.strip())
            elif line.strip() and not line.startswith('--'):
                cleaned.append(line.strip())
        
        # Join and normalize
        sig = ' '.join(cleaned)
        sig = re.sub(r'\s+', ' ', sig)
        sig = re.sub(r'\s*([,;()])\s*', r'\1', sig)
        
        return sig[:200] if sig else None  # Truncate long signatures
    
    def build_call_graph(self, system_name):
        """Build call graph for IPC functions"""
        path = self.systems[system_name]
        G = nx.DiGraph()
        
        # Use cscope if available
        cscope_db = path / 'cscope.out'
        if not cscope_db.exists():
            # Generate cscope database
            cmd = f"cd {path} && cscope -Rb 2>/dev/null"
            subprocess.run(cmd, shell=True, capture_output=True)
        
        # Extract function calls
        ipc_funcs = ['mach_msg', 'port_allocate', 'task_create', 'thread_create']
        
        for func in ipc_funcs:
            # Find who calls this function
            cmd = f"cd {path} && cscope -dL3 {func} 2>/dev/null | cut -d' ' -f2 | uniq"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            
            if result.stdout:
                callers = result.stdout.strip().split('\n')
                for caller in callers[:10]:  # Limit for visualization
                    if caller and caller != func:
                        G.add_edge(caller, func)
        
        self.call_graphs[system_name] = G
        return G
    
    def compare_interfaces(self):
        """Compare IPC interfaces across systems"""
        comparison = defaultdict(dict)
        
        # Get all unique functions across systems
        all_functions = set()
        for system in self.interfaces:
            if 'function_signatures' in self.interfaces[system]:
                all_functions.update(self.interfaces[system]['function_signatures'].keys())
        
        # Build comparison matrix
        for func in all_functions:
            for system in self.systems:
                if system in self.interfaces:
                    sigs = self.interfaces[system].get('function_signatures', {})
                    comparison[func][system] = 'Present' if func in sigs else 'Missing'
        
        return comparison
    
    def calculate_compatibility_score(self, sys1, sys2):
        """Calculate compatibility score between two systems"""
        if sys1 not in self.interfaces or sys2 not in self.interfaces:
            return 0
        
        sigs1 = set(self.interfaces[sys1].get('function_signatures', {}).keys())
        sigs2 = set(self.interfaces[sys2].get('function_signatures', {}).keys())
        
        if not sigs1 or not sigs2:
            return 0
        
        intersection = len(sigs1 & sigs2)
        union = len(sigs1 | sigs2)
        
        return (intersection / union) * 100 if union > 0 else 0
    
    def visualize_call_graphs(self):
        """Create visualization of call graphs"""
        fig, axes = plt.subplots(2, 2, figsize=(15, 15))
        axes = axes.flatten()
        
        for idx, (system, G) in enumerate(self.call_graphs.items()):
            if idx < 4 and len(G.nodes()) > 0:
                ax = axes[idx]
                pos = nx.spring_layout(G)
                nx.draw(G, pos, ax=ax, with_labels=True, 
                       node_color='lightblue', 
                       node_size=1500,
                       font_size=8,
                       arrows=True,
                       edge_color='gray')
                ax.set_title(f"{system} IPC Call Graph")
        
        plt.tight_layout()
        plt.savefig(self.base_path / 'ipc_call_graphs.png', dpi=150)
        print(f"Call graphs saved to {self.base_path / 'ipc_call_graphs.png'}")
    
    def generate_synthesis_plan(self):
        """Generate a plan for synthesizing IPC mechanisms"""
        plan = {
            'unified_interface': {},
            'conflicts': [],
            'recommendations': []
        }
        
        # Analyze commonalities
        comparison = self.compare_interfaces()
        
        for func, systems in comparison.items():
            present_in = [s for s, status in systems.items() if status == 'Present']
            
            if len(present_in) >= 3:
                plan['unified_interface'][func] = 'Core - present in most systems'
            elif len(present_in) == 2:
                plan['unified_interface'][func] = f'Optional - in {", ".join(present_in)}'
            elif len(present_in) == 1:
                plan['unified_interface'][func] = f'System-specific - only in {present_in[0]}'
        
        # Identify conflicts
        for sys1 in self.systems:
            for sys2 in self.systems:
                if sys1 < sys2:
                    score = self.calculate_compatibility_score(sys1, sys2)
                    if score < 50:
                        plan['conflicts'].append({
                            'systems': [sys1, sys2],
                            'compatibility': f'{score:.1f}%',
                            'severity': 'High' if score < 25 else 'Medium'
                        })
        
        # Generate recommendations
        plan['recommendations'] = [
            "1. Use CMU-Mach-MK83 as base for core IPC (most complete)",
            "2. Integrate Lites BSD compatibility layer for Unix IPC",
            "3. Add Mach4 real-time extensions for priority handling",
            "4. Use GNU-OSFMach for modern compiler compatibility",
            "5. Create abstraction layer to unify different IPC models",
            "6. Implement compatibility shims for missing functions"
        ]
        
        return plan
    
    def run_analysis(self):
        """Run complete IPC analysis"""
        print("="*80)
        print("IPC MECHANISM ANALYSIS")
        print("="*80)
        
        # Extract interfaces for each system
        for system in self.systems:
            print(f"\nAnalyzing {system}...")
            self.extract_mach_messages(system)
            self.extract_function_signatures(system)
            self.build_call_graph(system)
        
        # Compare interfaces
        print("\n" + "="*80)
        print("INTERFACE COMPARISON")
        print("="*80)
        
        comparison = self.compare_interfaces()
        
        # Print comparison matrix
        systems_list = list(self.systems.keys())
        print(f"\n{'Function':<25} {' '.join([s[:12] for s in systems_list])}")
        print("-" * 80)
        
        for func, systems in list(comparison.items())[:20]:  # Show first 20
            row = f"{func[:24]:<25}"
            for sys in systems_list:
                status = systems.get(sys, 'Missing')
                symbol = '✓' if status == 'Present' else '✗'
                row += f" {symbol:<12}"
            print(row)
        
        # Calculate compatibility scores
        print("\n" + "="*80)
        print("COMPATIBILITY MATRIX")
        print("="*80)
        
        for sys1 in self.systems:
            scores = []
            for sys2 in self.systems:
                score = self.calculate_compatibility_score(sys1, sys2)
                scores.append(f"{score:5.1f}%")
            print(f"{sys1:<15} {' '.join(scores)}")
        
        # Generate synthesis plan
        plan = self.generate_synthesis_plan()
        
        print("\n" + "="*80)
        print("SYNTHESIS PLAN")
        print("="*80)
        
        print("\nUnified Interface Components:")
        for func, status in list(plan['unified_interface'].items())[:15]:
            print(f"  {func:<25} - {status}")
        
        print("\nConflicts Identified:")
        for conflict in plan['conflicts']:
            print(f"  {' vs '.join(conflict['systems'])}: {conflict['compatibility']} compatibility ({conflict['severity']} severity)")
        
        print("\nRecommendations:")
        for rec in plan['recommendations']:
            print(f"  {rec}")
        
        # Save results
        with open(self.base_path / 'ipc_analysis.json', 'w') as f:
            json.dump({
                'interfaces': dict(self.interfaces),
                'comparison': dict(comparison),
                'synthesis_plan': plan
            }, f, indent=2)
        
        print(f"\nDetailed results saved to {self.base_path / 'ipc_analysis.json'}")
        
        # Visualize if we have matplotlib
        try:
            self.visualize_call_graphs()
        except Exception as e:
            print(f"Could not generate visualization: {e}")

if __name__ == "__main__":
    mapper = IPCMapper("/Users/eirikr/1_Workspace/Synthesis")
    mapper.run_analysis()