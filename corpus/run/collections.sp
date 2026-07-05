var scores = [90, 85, 77]
scores.push(100)
say scores
say scores.sort()
say scores.length()

var ages = {"Ada": 12, "Lin": 11}
ages["Sam"] = 13
say ages.keys()
say ages["Sam"]

# Value semantics: b is a copy the moment it changes.
var a = [1, 2]
var b = a
b.push(3)
say a
say b

var grid = [[1, 2], [3, 4]]
grid[0][1] = 9
say grid
