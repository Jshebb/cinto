import sys
import json
from collections import defaultdict

def main():
    if len(sys.argv) < 2:
        print("Usage: python summarize_run.py <results.jsonl>")
        sys.exit(1)
        
    path = sys.argv[1]
    
    total = 0
    passed = 0
    category_total = defaultdict(int)
    category_passed = defaultdict(int)
    total_tokens = 0
    total_duration = 0
    
    with open(path, 'r') as f:
        for line in f:
            if not line.strip(): continue
            data = json.loads(line)
            total += 1
            
            # Category is prefix of task_id (e.g. refactor_001 -> refactor)
            cat = data['task_id'].split('_')[0]
            category_total[cat] += 1
            
            outputs = data['outputs']
            
            is_success = (
                outputs.get('parsed_successfully', False) and
                outputs.get('required_slots_present', False) and
                outputs.get('filepaths_valid', False)
            )
            if 'code_compiles' in outputs and outputs['code_compiles'] is not None:
                is_success = is_success and outputs['code_compiles']
            if 'tests_pass' in outputs and outputs['tests_pass'] is not None:
                is_success = is_success and outputs['tests_pass']
                
            if is_success:
                passed += 1
                category_passed[cat] += 1
                
            metrics = data['metrics']
            total_tokens += metrics.get('tokens_out', 0)
            total_duration += metrics.get('duration_ms', 0)
            
    print(f"--- RESULTS FOR {path} ---")
    print(f"Total Tasks: {total}")
    print(f"Passed: {passed} ({passed/total*100:.1f}%)")
    print("----------------------------")
    for cat in sorted(category_total.keys()):
        t = category_total[cat]
        p = category_passed[cat]
        print(f"  {cat}: {p}/{t} ({p/t*100:.1f}%)")
    print("----------------------------")
    if total > 0:
        print(f"Avg tokens out: {total_tokens/total:.1f}")
        print(f"Avg duration ms: {total_duration/total:.1f}")
    
if __name__ == '__main__':
    main()
