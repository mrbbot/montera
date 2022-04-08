import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from helpers import __dirname, PROJECT_NAMES, PROJECT_COLOURS, latest_output_path

pd.set_option('display.max_rows', None)

csv_path = latest_output_path("size")
df = pd.read_csv(csv_path)

# Format project names
df = df.replace(PROJECT_NAMES)

# Sort columns by lowest-to-highest mean compilation time
index = df["bytes"].sort_values(ascending=False).index
df = df.reindex(index)

# Plot bar chart
color = [PROJECT_COLOURS[df["name"][i]] for i in index]
ax = df.plot(kind="bar", x="name", y="bytes", logy=True, legend=False, xlabel="", ylabel="Download Size (bytes)", color=color)

# Add vertical padding to top of chart to fit value labels
y_min, y_max = ax.get_ylim()
y_max *= 1.6
ax.set_ylim(y_min, y_max)

# Add value labels to bars: https://stackoverflow.com/a/34598688
for p in ax.patches:
    s = ""
    bytes = p.get_height()
    if bytes < 1000:
        s = f"{bytes}B"
    elif bytes < 1000000:
        s = f"{bytes/1000:.1f}KB"
    else:
        s = f"{bytes/1000000:.1f}MB"
    ax.annotate(s, xy=(p.get_x() + p.get_width() / 2.0, p.get_height()), ha="center", va="center", xytext=(0, 10), textcoords="offset points")
plt.xticks(rotation=30, ha="right")
plt.tight_layout()
plt.savefig(__dirname + "/size.pdf")
plt.show()