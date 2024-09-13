#!/usr/bin/env python3
import sys
import statistics

from pathlib import Path


def load_app(app):

    results = []

    # Go through all targets in the edgcount directory
    for target in app.glob("edgecount/*"):
        # Get the number from the file
        with open(target, "r") as f:
            num_edges_withcov = int(f.read().strip())

        # Construct the path to the file with the number of edges
        target_nocov = target.parent.parent / "edgecount-nocov" / target.name

        # Skip if the file does not exist
        if not target_nocov.exists():
            continue

        # Get the number of edges from the file
        with open(target_nocov, "r") as f:
            num_edges_nocov = int(f.read().strip())

        if num_edges_nocov > num_edges_withcov:
            print(target, num_edges_withcov, num_edges_nocov)

        if 'androidx.work.impl.diagnostics.DiagnosticsReceiver' in str(target):
            continue

        if num_edges_withcov/num_edges_nocov > 1000:
            # Exclude outlier
            continue

        #print(target, num_edges_withcov, num_edges_nocov)
        results.append((num_edges_withcov, num_edges_nocov))

    return results


def analysis_per_target(data):
    average_improvements = list(map(lambda x: x[0] / x[1], data))
    average_improvements.sort()

    print("Average improvement:", statistics.mean(average_improvements))
    print("Median improvement:", statistics.median(average_improvements))
    print("Standard deviation:", statistics.stdev(average_improvements))
    print("25 percentile:", statistics.quantiles(average_improvements)[0])
    print("75 percentile:", statistics.quantiles(average_improvements)[2])


def analysis_per_app(data):
    # Print percentage apps where at least one target has an improvement
    count = 0
    for app in data:
        for target in app:
            if target[0] < target[1]:
                count += 1
                break

    print("Percentage apps with improvement:", count / len(data))

    


def main():
    data = []

    # All apps are in this directory
    for edgecount_dir in Path(".").glob("*/edgecount/"):
        app = edgecount_dir.parent
        #print(app)

        # Load the data for this app
        data.append(load_app(app))

    analysis_per_target([item for sublist in data for item in sublist])
    analysis_per_app(data)


if __name__ == "__main__":
    main()
