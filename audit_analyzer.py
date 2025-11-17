#!/usr/bin/env python3
"""
Synthesis OS Audit Analyzer
Identifies gaps, missing dependencies, and build issues
"""

import os
import re
import subprocess
from pathlib import Path
from collections import defaultdict
import json

class AuditAnalyzer:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.merged_path = self.base_path / 'merged'
        self.gaps = {
            'missing_implementations': [],
            'undefined_symbols': [],
            'header_issues': [],
            'link_errors': [],
            'missing_deps': []
        }
        self.header_deps = defaultdict(set)
        self.source_headers = defaultdict(set)
        self.todos = []
        
    def analyze_headers(self):
        """Analyze all header files for dependencies"""
        print("Analyzing header dependencies...")
        headers = list(self.merged_path.rglob('*.h'))
        
        for header in headers:
            with open(header, 'r', encoding='latin-1') as f:
                content = f.read()
                
            # Find includes
            includes = re.findall(r'#include\s+[<"]([^>"]+)[>"]', content)
            rel_path = header.relative_to(self.merged_path)
            self.header_deps[str(rel_path)] = set(includes)
            
            # Find function declarations without implementations
            func_decls = re.findall(r'^(?:extern\s+)?(?:\w+\s+)+(\w+)\s*\([^)]*\);', content, re.MULTILINE)
            
            for func in func_decls:
                # Check if implementation exists
                if not self.find_implementation(func):
                    self.gaps['missing_implementations'].append({
                        'function': func,
                        'header': str(rel_path)
                    })
    
    def find_implementation(self, func_name):
        """Search for function implementation"""
        cmd = f"grep -r '^[^/]*{func_name}\\s*(' {self.merged_path} --include='*.c' 2>/dev/null | head -1"
        result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        return bool(result.stdout.strip())
    
    def analyze_sources(self):
        """Analyze source files for missing headers"""
        print("Analyzing source files...")
        sources = list(self.merged_path.rglob('*.c'))
        
        for source in sources:
            with open(source, 'r', encoding='latin-1') as f:
                content = f.read()
            
            # Find includes
            includes = re.findall(r'#include\s+[<"]([^>"]+)[>"]', content)
            rel_path = source.relative_to(self.merged_path)
            self.source_headers[str(rel_path)] = set(includes)
            
            # Check if includes exist
            for inc in includes:
                inc_path = self.merged_path / 'include' / inc
                if not inc_path.exists():
                    # Try alternate paths
                    alt_paths = [
                        self.merged_path / inc,
                        self.merged_path / 'include' / 'mach' / inc,
                        self.merged_path / 'include' / 'sys' / inc
                    ]
                    if not any(p.exists() for p in alt_paths):
                        self.gaps['header_issues'].append({
                            'source': str(rel_path),
                            'missing_header': inc
                        })
            
            # Find undefined functions
            func_calls = re.findall(r'\b(\w+)\s*\([^)]*\)', content)
            for func in func_calls:
                if func not in ['if', 'while', 'for', 'switch', 'return', 'sizeof']:
                    # Check if declared
                    if not self.is_function_declared(func, includes):
                        self.gaps['undefined_symbols'].append({
                            'source': str(rel_path),
                            'symbol': func
                        })
    
    def is_function_declared(self, func, includes):
        """Check if function is declared in included headers"""
        # Check standard functions
        stdlib_funcs = ['malloc', 'free', 'printf', 'memcpy', 'strcpy', 'strlen']
        if func in stdlib_funcs:
            return True
            
        # Search in project headers
        for inc in includes:
            header_paths = [
                self.merged_path / 'include' / inc,
                self.merged_path / 'include' / 'mach' / inc,
                self.merged_path / 'include' / 'sys' / inc
            ]
            
            for path in header_paths:
                if path.exists():
                    with open(path, 'r', encoding='latin-1') as f:
                        if func in f.read():
                            return True
        return False
    
    def check_build_consistency(self):
        """Check if Makefile references all source files"""
        print("Checking build consistency...")
        makefile = self.merged_path / 'Makefile'
        
        if makefile.exists():
            with open(makefile, 'r') as f:
                makefile_content = f.read()
            
            # Find all .c files
            all_sources = list(self.merged_path.rglob('*.c'))
            
            for source in all_sources:
                rel_path = source.relative_to(self.merged_path)
                basename = source.stem
                
                # Check if referenced in Makefile
                if basename not in makefile_content and str(rel_path) not in makefile_content:
                    self.gaps['missing_deps'].append({
                        'file': str(rel_path),
                        'issue': 'Not referenced in Makefile'
                    })
    
    def analyze_link_dependencies(self):
        """Analyze linking dependencies"""
        print("Analyzing link dependencies...")
        
        # Map symbols to files
        symbol_map = defaultdict(list)
        
        for source in self.merged_path.rglob('*.c'):
            cmd = f"nm -U {source} 2>/dev/null | grep ' T ' | awk '{{print $3}}'"
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            
            if result.stdout:
                for symbol in result.stdout.strip().split('\n'):
                    if symbol:
                        symbol_map[symbol].append(str(source.relative_to(self.merged_path)))
        
        # Check for duplicate symbols
        for symbol, files in symbol_map.items():
            if len(files) > 1:
                self.gaps['link_errors'].append({
                    'symbol': symbol,
                    'duplicated_in': files
                })
    
    def generate_todos(self):
        """Generate granular TODO list"""
        print("\nGenerating TODO list...")
        
        # Priority 1: Missing headers
        for issue in self.gaps['header_issues'][:10]:
            self.todos.append({
                'priority': 1,
                'category': 'HEADER',
                'task': f"Create or locate header: {issue['missing_header']}",
                'file': issue['source']
            })
        
        # Priority 2: Missing implementations
        for impl in self.gaps['missing_implementations'][:20]:
            self.todos.append({
                'priority': 2,
                'category': 'IMPLEMENTATION',
                'task': f"Implement function: {impl['function']}",
                'file': impl['header']
            })
        
        # Priority 3: Undefined symbols
        unique_symbols = set()
        for sym in self.gaps['undefined_symbols']:
            if sym['symbol'] not in unique_symbols:
                unique_symbols.add(sym['symbol'])
                self.todos.append({
                    'priority': 3,
                    'category': 'SYMBOL',
                    'task': f"Declare or implement: {sym['symbol']}",
                    'file': sym['source']
                })
                if len(unique_symbols) >= 15:
                    break
        
        # Priority 4: Build system
        for dep in self.gaps['missing_deps'][:10]:
            self.todos.append({
                'priority': 4,
                'category': 'BUILD',
                'task': f"Add to Makefile: {dep['file']}",
                'issue': dep['issue']
            })
        
        # Priority 5: Link issues
        for link in self.gaps['link_errors'][:5]:
            self.todos.append({
                'priority': 5,
                'category': 'LINK',
                'task': f"Resolve duplicate symbol: {link['symbol']}",
                'files': link['duplicated_in']
            })
    
    def generate_dependency_graph(self):
        """Generate header dependency graph"""
        print("Generating dependency graph...")
        
        dot_content = ["digraph dependencies {"]
        dot_content.append('  rankdir=LR;')
        dot_content.append('  node [shape=box];')
        
        for header, deps in self.header_deps.items():
            header_name = Path(header).name
            for dep in deps:
                dep_name = Path(dep).name
                dot_content.append(f'  "{header_name}" -> "{dep_name}";')
        
        dot_content.append("}")
        
        dot_file = self.base_path / 'dependencies.dot'
        with open(dot_file, 'w') as f:
            f.write('\n'.join(dot_content))
        
        # Generate image if graphviz available
        cmd = f"dot -Tpng {dot_file} -o {self.base_path}/dependencies.png 2>/dev/null"
        subprocess.run(cmd, shell=True)
    
    def generate_report(self):
        """Generate comprehensive audit report"""
        report = []
        report.append("="*80)
        report.append("SYNTHESIS OS AUDIT REPORT")
        report.append("="*80)
        report.append("")
        
        # Statistics
        total_headers = len(list(self.merged_path.rglob('*.h')))
        total_sources = len(list(self.merged_path.rglob('*.c')))
        
        report.append(f"Files Analyzed:")
        report.append(f"  Headers (.h): {total_headers}")
        report.append(f"  Sources (.c): {total_sources}")
        report.append("")
        
        # Gap Analysis
        report.append("Gap Analysis:")
        report.append(f"  Missing Headers: {len(self.gaps['header_issues'])}")
        report.append(f"  Missing Implementations: {len(self.gaps['missing_implementations'])}")
        report.append(f"  Undefined Symbols: {len(self.gaps['undefined_symbols'])}")
        report.append(f"  Link Errors: {len(self.gaps['link_errors'])}")
        report.append(f"  Build Issues: {len(self.gaps['missing_deps'])}")
        report.append("")
        
        # Critical Issues
        report.append("CRITICAL ISSUES:")
        report.append("-" * 40)
        
        if self.gaps['header_issues']:
            report.append("\n1. Missing Headers (Top 5):")
            for issue in self.gaps['header_issues'][:5]:
                report.append(f"   - {issue['missing_header']} needed by {issue['source']}")
        
        if self.gaps['missing_implementations']:
            report.append("\n2. Missing Implementations (Top 5):")
            for impl in self.gaps['missing_implementations'][:5]:
                report.append(f"   - {impl['function']} declared in {impl['header']}")
        
        if self.gaps['link_errors']:
            report.append("\n3. Link Conflicts (Top 3):")
            for link in self.gaps['link_errors'][:3]:
                report.append(f"   - {link['symbol']} duplicated in {len(link['duplicated_in'])} files")
        
        # TODO List
        report.append("\n" + "="*80)
        report.append("GRANULAR TODO LIST")
        report.append("="*80)
        
        for i, todo in enumerate(sorted(self.todos, key=lambda x: x['priority'])[:30], 1):
            report.append(f"\n{i}. [{todo['category']}] Priority {todo['priority']}")
            report.append(f"   Task: {todo['task']}")
            if 'file' in todo:
                report.append(f"   File: {todo['file']}")
            if 'files' in todo:
                report.append(f"   Files: {', '.join(todo['files'][:3])}")
        
        report_text = '\n'.join(report)
        print(report_text)
        
        # Save report
        with open(self.base_path / 'AUDIT_REPORT.txt', 'w') as f:
            f.write(report_text)
        
        # Save JSON for processing
        with open(self.base_path / 'audit_gaps.json', 'w') as f:
            json.dump({
                'gaps': self.gaps,
                'todos': self.todos,
                'header_deps': dict(self.header_deps),
                'source_headers': dict(self.source_headers)
            }, f, indent=2)
    
    def run_audit(self):
        """Run complete audit"""
        print("Starting Synthesis OS Audit...")
        print("="*80)
        
        self.analyze_headers()
        self.analyze_sources()
        self.check_build_consistency()
        # self.analyze_link_dependencies()  # Skip if nm not available
        self.generate_todos()
        self.generate_dependency_graph()
        self.generate_report()
        
        print(f"\nAudit complete! Results saved to {self.base_path}")
        print(f"  - AUDIT_REPORT.txt: Human-readable report")
        print(f"  - audit_gaps.json: Machine-readable gaps")
        print(f"  - dependencies.dot: Dependency graph")

if __name__ == "__main__":
    auditor = AuditAnalyzer("/Users/eirikr/1_Workspace/Synthesis")
    auditor.run_audit()