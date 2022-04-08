import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from helpers import __dirname, PROJECT_NAMES, PROJECT_COLOURS, latest_output_path

pd.set_option('display.max_rows', None)

csv_path = latest_output_path("build")
df = pd.read_csv(csv_path)

# Skip first 3 iterations
df = df[df["iteration"] >= 3]

# Extract project names and build times
df = df[["name", "build_time_ms"]]

# Format project names
df = df.replace(PROJECT_NAMES)

# Convert milliseconds to seconds
df["build_time_ms"] /= 1000

# Calculate mean and standard deviation for each project
df = df.groupby("name").agg([np.mean, np.std])["build_time_ms"]

# Sort columns by lowest-to-highest mean compilation time
index = df["mean"].sort_values(ascending=False).index
df = df.reindex(index)

# Plot bar chart
color = [PROJECT_COLOURS[name] for name in index]
ax = df.plot(kind="bar", y="mean", yerr="std", legend=False, xlabel="", ylabel="Compilation Time (s)", capsize=4, color=color) # title="Compilation Time",
# Add value labels to bars: https://stackoverflow.com/a/34598688
for p in ax.patches:
    ax.annotate("%.2f" % p.get_height(), xy=(p.get_x() + p.get_width() / 2.0, p.get_height()), ha="center", va="center", xytext=(0, 10), textcoords="offset points")
plt.xticks(rotation=30, ha="right")
plt.tight_layout()
plt.savefig(__dirname + "/build.pdf")
plt.show()