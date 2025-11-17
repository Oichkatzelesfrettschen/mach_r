#!/usr/bin/env python3
"""
Fix missing headers in synthesized OS
"""

import os
import shutil
from pathlib import Path

class HeaderFixer:
    def __init__(self, base_path):
        self.base_path = Path(base_path)
        self.merged_path = self.base_path / 'merged'
        self.source_systems = {
            'mach': self.base_path / 'CMU-Mach-MK83',
            'lites': self.base_path / 'Lites-1.1/lites-1.1',
            'mach4': self.base_path / 'mach4-i386/mach4-i386',
            'gnu': self.base_path / 'gnu-osfmach/gnu-osfmach'
        }
        
    def find_and_copy_header(self, header_name):
        """Find header in source systems and copy to merged"""
        for system, path in self.source_systems.items():
            cmd = f"find {path} -name '{header_name}' -type f 2>/dev/null | head -1"
            import subprocess
            result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
            
            if result.stdout.strip():
                source_file = Path(result.stdout.strip())
                
                # Determine destination
                if 'mach' in header_name or 'ipc' in header_name:
                    dest_dir = self.merged_path / 'include' / 'mach'
                elif 'kern' in str(source_file):
                    dest_dir = self.merged_path / 'include' / 'kern'
                else:
                    dest_dir = self.merged_path / 'include' / 'sys'
                
                dest_dir.mkdir(parents=True, exist_ok=True)
                dest_file = dest_dir / header_name
                
                shutil.copy2(source_file, dest_file)
                print(f"  Copied {header_name} from {system} to {dest_file.relative_to(self.merged_path)}")
                return True
        return False
    
    def create_missing_kern_headers(self):
        """Create essential kern headers"""
        kern_dir = self.merged_path / 'include' / 'kern'
        kern_dir.mkdir(parents=True, exist_ok=True)
        
        # Create lock.h
        lock_h = """#ifndef _KERN_LOCK_H_
#define _KERN_LOCK_H_

typedef struct lock {
    int locked;
    void *holder;
} lock_t;

#define lock_init(l) ((l)->locked = 0)
#define lock_lock(l) while((l)->locked) ; (l)->locked = 1
#define lock_unlock(l) ((l)->locked = 0)

#endif /* _KERN_LOCK_H_ */
"""
        (kern_dir / 'lock.h').write_text(lock_h)
        print("  Created kern/lock.h")
        
        # Create mach_param.h
        mach_param_h = """#ifndef _KERN_MACH_PARAM_H_
#define _KERN_MACH_PARAM_H_

#define TASK_MAX 1024
#define THREAD_MAX 4096
#define PORT_MAX 8192
#define SET_MAX 1024
#define ITE_MAX 64
#define SPACE_MAX 1024
#define NCPUS 32

#endif /* _KERN_MACH_PARAM_H_ */
"""
        (kern_dir / 'mach_param.h').write_text(mach_param_h)
        print("  Created kern/mach_param.h")
        
        # Create kalloc.h
        kalloc_h = """#ifndef _KERN_KALLOC_H_
#define _KERN_KALLOC_H_

#include <sys/types.h>

void *kalloc(size_t size);
void kfree(void *ptr, size_t size);
void kalloc_init(void);

#endif /* _KERN_KALLOC_H_ */
"""
        (kern_dir / 'kalloc.h').write_text(kalloc_h)
        print("  Created kern/kalloc.h")
        
        # Create zalloc.h
        zalloc_h = """#ifndef _KERN_ZALLOC_H_
#define _KERN_ZALLOC_H_

typedef struct zone *zone_t;

zone_t zinit(size_t size, int max, int alloc, char *name);
void *zalloc(zone_t zone);
void zfree(zone_t zone, void *elem);

#endif /* _KERN_ZALLOC_H_ */
"""
        (kern_dir / 'zalloc.h').write_text(zalloc_h)
        print("  Created kern/zalloc.h")
    
    def fix_ipc_headers(self):
        """Fix IPC-related headers"""
        print("Fixing IPC headers...")
        
        # Headers to find and copy
        ipc_headers = [
            'ipc_pset.h', 'ipc_marequest.h', 'ipc_table.h', 
            'ipc_thread.h', 'ipc_hash.h', 'ipc_object.h',
            'ipc_init.h', 'mach_debug.h'
        ]
        
        for header in ipc_headers:
            self.find_and_copy_header(header)
    
    def create_mach_compatibility_header(self):
        """Create mach_ipc_compat.h"""
        compat_h = """#ifndef _MACH_IPC_COMPAT_H_
#define _MACH_IPC_COMPAT_H_

/* Compatibility definitions for Mach IPC */

#define MSG_TYPE_NORMAL 0
#define MSG_TYPE_EMERGENCY 1
#define MSG_TYPE_PORT 2
#define MSG_TYPE_PORT_ONCE 3
#define MSG_TYPE_PORT_NAME 4

typedef struct {
    unsigned int msgt_name : 8;
    unsigned int msgt_size : 8;
    unsigned int msgt_number : 12;
    unsigned int msgt_inline : 1;
    unsigned int msgt_longform : 1;
    unsigned int msgt_deallocate : 1;
    unsigned int msgt_unused : 1;
} msg_type_t;

#define MACH_MSG_TYPE_MOVE_RECEIVE 16
#define MACH_MSG_TYPE_MOVE_SEND 17
#define MACH_MSG_TYPE_MOVE_SEND_ONCE 18
#define MACH_MSG_TYPE_COPY_SEND 19
#define MACH_MSG_TYPE_MAKE_SEND 20
#define MACH_MSG_TYPE_MAKE_SEND_ONCE 21

#endif /* _MACH_IPC_COMPAT_H_ */
"""
        header_path = self.merged_path / 'include' / 'mach' / 'mach_ipc_compat.h'
        header_path.write_text(compat_h)
        print("  Created mach/mach_ipc_compat.h")
    
    def run_fixes(self):
        """Run all header fixes"""
        print("Fixing missing headers...")
        print("-" * 40)
        
        # Create missing directories
        (self.merged_path / 'include' / 'ipc').mkdir(parents=True, exist_ok=True)
        
        # Fix critical headers
        self.create_missing_kern_headers()
        self.create_mach_compatibility_header()
        self.fix_ipc_headers()
        
        # Copy missing Mach headers
        mach_headers = ['message.h', 'port.h']
        for header in mach_headers:
            if not (self.merged_path / 'include' / 'mach' / header).exists():
                self.find_and_copy_header(header)
        
        print("\nHeader fixes complete!")

if __name__ == "__main__":
    fixer = HeaderFixer("/Users/eirikr/1_Workspace/Synthesis")
    fixer.run_fixes()