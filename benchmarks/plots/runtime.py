import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from helpers import __dirname, PROJECT_NAMES, PROJECT_COLOURS, latest_output_path

pd.set_option('display.max_rows', None)

csv_path = latest_output_path("runtime")
df = pd.read_csv(csv_path)

# Skip first 3 iterations
df = df[df["iteration"] >= 3]

# Extract project names and build times
df = df[["name", "fib_time_ms", "gcd_time_ms", "sum_time_ms"]]

# Format project names
df = df.replace(PROJECT_NAMES)

# Convert milliseconds to seconds
df["fib_time_ms"] /= 1000
df["gcd_time_ms"] /= 1000
df["sum_time_ms"] /= 1000

# Calculate mean and standard deviation for each project
df = df.groupby("name").agg([np.mean, np.std])

# Plot bar charts
index = df["fib_time_ms"]["mean"].sort_values(ascending=False).index
df = df.reindex(index)
color = [PROJECT_COLOURS[name] for name in index]
df["fib_time_ms"].plot(kind="bar", y="mean", yerr="std", legend=False, xlabel="", ylabel="Execution Time (s)", capsize=4, color=color, title="Fibonacci"),
plt.xticks(rotation=30, ha="right")
plt.tight_layout()
plt.savefig(__dirname + "/runtime-fib.pdf")

index = df["gcd_time_ms"]["mean"].sort_values(ascending=False).index
df = df.reindex(index)
color = [PROJECT_COLOURS[name] for name in index]
df["gcd_time_ms"].plot(kind="bar", y="mean", logy=True, yerr="std", legend=False, xlabel="", ylabel="Execution Time (s)", capsize=4, color=color, title="GCD"),
plt.xticks(rotation=30, ha="right")
plt.tight_layout()
plt.savefig(__dirname + "/runtime-gcd.pdf")

index = df["sum_time_ms"]["mean"].sort_values(ascending=False).index
df = df.reindex(index)
color = [PROJECT_COLOURS[name] for name in index]
df["sum_time_ms"].plot(kind="bar", y="mean", logy=True, yerr="std", legend=False, xlabel="", ylabel="Execution Time (s)", capsize=4, color=color, title="Object Sum"),
plt.xticks(rotation=30, ha="right")
plt.tight_layout()
plt.savefig(__dirname + "/runtime-sum.pdf")

plt.show()