import os
import json
import shutil
import random

DATASET_DIR = "eval/tasks/v1/expanded"
TASKS_JSONL = os.path.join(DATASET_DIR, "tasks.jsonl")

# Templates
CARGO_TOML_TEMPLATE = """[package]
name = "{name}"
version = "0.1.0"
edition = "2021"
"""

def create_fixture(task_id, code, test_code, prompt, validation_command="cargo test"):
    initial_dir = os.path.join(DATASET_DIR, task_id, "initial")
    src_dir = os.path.join(initial_dir, "src")
    os.makedirs(src_dir, exist_ok=True)
    
    with open(os.path.join(initial_dir, "Cargo.toml"), "w") as f:
        f.write(CARGO_TOML_TEMPLATE.format(name=task_id))
        
    with open(os.path.join(src_dir, "lib.rs"), "w") as f:
        f.write(code + "\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n" + test_code + "\n}")

    return {
        "id": task_id,
        "prompt": prompt,
        "fixture_dir": initial_dir,
        "validation_command": validation_command
    }

def generate_refactor_tasks():
    tasks = []
    for i in range(1, 51):
        task_id = f"refactor_{i:03d}"
        var_name = random.choice(["data", "values", "numbers", "items", "measurements"])
        func_name = f"process_{var_name}"
        helper_name = f"calculate_metric_{i}"
        
        code = f"""pub fn {func_name}({var_name}: &[i32]) -> i32 {{
    let mut sum = 0;
    for v in {var_name} {{
        sum += v;
    }}
    let avg = if {var_name}.is_empty() {{ 0 }} else {{ sum / {var_name}.len() as i32 }};
    
    // Some complex stuff
    let mut result = 0;
    for v in {var_name} {{
        result += (v - avg) * {i};
    }}
    result
}}"""
        test_code = f"""    #[test]
    fn test_process() {{
        assert_eq!({func_name}(&[1, 2, 3]), {func_name}(&[1, 2, 3])); // Identity test to ensure behavior preserves
    }}"""
        prompt = f"The `{func_name}` function in src/lib.rs is too long. Extract the average calculation logic (the `sum` and `avg` part) into a separate helper function called `{helper_name}` that takes a slice of i32 and returns an i32."
        
        tasks.append(create_fixture(task_id, code, test_code, prompt))
    return tasks

def generate_feature_tasks():
    tasks = []
    for i in range(1, 51):
        task_id = f"feature_{i:03d}"
        struct_name = f"Entity{i}"
        field_name = f"property_{i}"
        
        code = f"""pub struct {struct_name} {{
    id: u32,
    {field_name}: String,
}}

impl {struct_name} {{
    pub fn new(id: u32, {field_name}: String) -> Self {{
        Self {{ id, {field_name} }}
    }}
}}"""
        test_code = f"""    #[test]
    fn test_getter() {{
        let e = {struct_name}::new(1, "test".to_string());
        assert_eq!(e.get_{field_name}(), "test");
    }}"""
        prompt = f"The struct `{struct_name}` in src/lib.rs needs a getter method for `{field_name}`. Add a method `pub fn get_{field_name}(&self) -> &str`."
        
        tasks.append(create_fixture(task_id, code, test_code, prompt))
    return tasks

def generate_bugfix_tasks():
    tasks = []
    for i in range(1, 51):
        task_id = f"bugfix_{i:03d}"
        func_name = f"analyze_{i}"
        
        code = f"""pub fn {func_name}(data: &[i32]) -> i32 {{
    if data.is_empty() {{ return 0; }}
    let mut max = data[0];
    for i in 1..=data.len() {{ // OFF BY ONE BUG
        if data[i] > max {{
            max = data[i];
        }}
    }}
    max
}}"""
        test_code = f"""    #[test]
    fn test_analyze() {{
        assert_eq!({func_name}(&[1, 5, 3]), 5);
    }}"""
        prompt = f"There is an out-of-bounds bug in the `{func_name}` function in src/lib.rs. Fix it so the test passes without panicking."
        
        tasks.append(create_fixture(task_id, code, test_code, prompt))
    return tasks

def generate_edge_tasks():
    tasks = []
    for i in range(1, 51):
        task_id = f"edge_{i:03d}"
        func_name = f"compute_{i}"
        
        code = f"""pub fn {func_name}(x: i32) -> i32 {{
    let a = x * 2
    let b = a + 5;
    b
}}"""
        test_code = f"""    #[test]
    fn test_compute() {{
        assert_eq!({func_name}(2), 9);
    }}"""
        prompt = f"Fix the syntax error in src/lib.rs. The file fails to compile."
        
        tasks.append(create_fixture(task_id, code, test_code, prompt, validation_command="cargo check"))
    return tasks

def main():
    if os.path.exists(DATASET_DIR):
        shutil.rmtree(DATASET_DIR)
    os.makedirs(DATASET_DIR, exist_ok=True)
    
    print("Generating tasks...")
    tasks = []
    tasks.extend(generate_refactor_tasks())
    tasks.extend(generate_feature_tasks())
    tasks.extend(generate_bugfix_tasks())
    tasks.extend(generate_edge_tasks())
    
    with open(TASKS_JSONL, "w") as f:
        for t in tasks:
            f.write(json.dumps(t) + "\n")
            
    print(f"Generated {len(tasks)} tasks in {DATASET_DIR}")

if __name__ == "__main__":
    main()
