def inc(x):
    return x + 1
xs = [1, 2, 3]
print(len([inc(x) for x in xs]))
