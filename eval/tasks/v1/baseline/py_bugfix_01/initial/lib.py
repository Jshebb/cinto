def find_max(data):
    max_val = data[0]
    for i in range(1, len(data) - 1):
        if data[i] > max_val:
            max_val = data[i]
    return max_val
