def process_data(data):
    total = 0
    for x in data:
        total += x
    average = total / len(data) if data else 0.0
    result = []
    for x in data:
        result.append(x - average)
    return result
