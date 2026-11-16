"""BCF の公開 API の観測を、入力のレシピから定めた値と照らす。"""

import base64
import hashlib
import math


def sha(data):
    return hashlib.sha256(data).hexdigest()


def expected(acc, version, mixed_cameras, snapshots):
    topics = []
    # expected_cases は入力の作成に使う定数であり、コアや外部実装の観測ではない。
    for number, item in enumerate(acc.expected_cases(None), 1):
        camera = dict(acc.CAMERA)
        if mixed_cameras and number % 2 == 0:
            camera = {"projection": "orthographic", "position": [-5, 1.5, 1.5], "direction": [1, 0, 0], "up": [0, 0, 1], "width": 12}
        view = acc.guid(number, "view")
        topics.append({"id": item["id"], "title": item["title"], "description": item["description"], "kind": "Issue",
                       "status": item["status"].title(), "author": acc.AUTHOR, "created": acc.DATE,
                       "comments": [{"id": acc.guid(number, "comment"), "author": acc.AUTHOR, "date": acc.DATE, "text": item["comment"], "location": view}],
                       "locations": [{"id": view, "camera": camera, "selected": item["selected"], "visible": [True, []],
                                      "aspectRatio": 1.0 if version == "3.0" else None, "snapshotSha256": sha(acc.snapshot()) if snapshots else None}]})
    return {"version": version, "topics": topics}


def vector(point):
    return [point[key] for key in ("X", "Y", "Z")]


def external_values(document):
    version = document["Version"]["VersionId"]
    if version not in ("2.1", "3.0"):
        raise ValueError("version")
    topics = []
    for markup in document["Markups"]:
        topic = markup["Topic"]
        views = markup.get("Viewpoints", []) if version == "2.1" else topic.get("Viewpoints", [])
        comments = markup.get("Comment", []) if version == "2.1" else topic.get("Comments", [])
        locations = []
        for view in views:
            data = view["VisualizationInfo"]
            perspective, orthographic = data.get("PerspectiveCamera"), data.get("OrthogonalCamera")
            if bool(perspective) == bool(orthographic):
                raise ValueError("camera")
            camera = perspective or orthographic
            shape = {"projection": "perspective" if perspective else "orthographic", "position": vector(camera["CameraViewPoint"]),
                     "direction": vector(camera["CameraDirection"]), "up": vector(camera["CameraUpVector"])}
            shape["field_of_view" if perspective else "width"] = camera["FieldOfView" if perspective else "ViewToWorldScale"]
            components = data["Components"]
            visibility = components["Visibility"]
            image = view.get("snapshot_data")
            locations.append({"id": view["Guid"], "visualizationId": data["Guid"], "camera": shape,
                              "selected": sorted(item["IfcGuid"] for item in components.get("Selection", [])),
                              "visible": [visibility["DefaultVisibility"], sorted(item["IfcGuid"] for item in visibility.get("Exceptions", []))],
                              "aspectRatio": camera.get("AspectRatio"), "snapshotSha256": sha(base64.b64decode(image["Data"], validate=True)) if image else None})
        for location in locations:
            if location.pop("visualizationId") != location["id"]:
                raise ValueError("viewpoint.reference")
        topics.append({"id": topic["Guid"], "title": topic["Title"], "description": topic["Description"], "kind": topic["TopicType"],
                       "status": topic["TopicStatus"], "author": topic["CreationAuthor"], "created": topic["CreationDate"],
                       "comments": sorted([{"id": item["Guid"], "author": item["Author"], "date": item["Date"],
                                            "text": item["CommentProperty"], "location": item["Viewpoint"]["Guid"]} for item in comments], key=lambda item: item["id"]),
                       "locations": sorted(locations, key=lambda item: item["id"])})
    return {"version": version, "topics": sorted(topics, key=lambda item: item["id"])}


def core_values(review, version):
    topics = []
    for topic in review["topics"]:
        locations = []
        for location in topic["locations"]:
            if location["kind"] != "3d":
                raise ValueError("location.kind")
            locations.append({"id": location["id"], "target": location["target"], "camera": location["camera"], "selected": sorted(item["id"] for item in location.get("selected", [])),
                              "visible": [location["visible"], []]})
        topics.append({"id": topic["id"], "title": topic["title"], "description": topic["description"], "kind": topic["kind"],
                       "status": topic.get("status_label", topic["status"].title()), "author": topic["author"], "created": topic["created"],
                       "comments": sorted([{key: item[key] for key in ("id", "author", "date", "text", "location")} for item in topic["comments"]], key=lambda item: item["id"]),
                       "locations": sorted(locations, key=lambda item: item["id"])})
    return {"version": version, "topics": sorted(topics, key=lambda item: item["id"])}


def differences(wanted, observed, path=""):
    # 座標は整数と実数の同じ値を許す。真偽値を数値として扱わない。
    if type(wanted) in (int, float) and type(observed) in (int, float):
        return [] if math.isfinite(observed) and wanted == observed else [path + ":value"]
    if type(wanted) is not type(observed):
        return [path + ":type"]
    if isinstance(wanted, dict):
        errors = [path + ":keys"] if wanted.keys() != observed.keys() else []
        for key in sorted(wanted.keys() & observed.keys()):
            errors.extend(differences(wanted[key], observed[key], path + "/" + key))
        return errors
    if isinstance(wanted, list):
        errors = [path + ":length"] if len(wanted) != len(observed) else []
        for index, (left, right) in enumerate(zip(wanted, observed)):
            errors.extend(differences(left, right, path + "/" + str(index)))
        return errors
    return [] if wanted == observed else [path + ":value"]


def compare_external(wanted, document):
    try:
        observed = external_values(document)
        errors = differences(wanted, observed)
        return {"matched": not errors, "differences": errors, "observed": observed}
    except (KeyError, TypeError, ValueError) as error:
        return {"matched": False, "differences": ["observation:" + type(error).__name__]}


def compare_core(wanted, response, review, reference):
    expected_response = {"v": 1, "result": {"version": wanted["version"], "topics": len(wanted["topics"])}, "diagnostics": []}
    actual_response = {key: value for key, value in response.items() if key != "blobs"}
    errors = differences(expected_response, actual_response, "response")
    errors.extend(differences(reference, {key: value for key, value in review.items() if key != "topics"}, "reference"))
    # Review のカメラには縦横比と画像を持たないので、画像保持は外部観測で確かめる。
    core_expected = {"version": wanted["version"], "topics": [
        {**topic, "locations": [{"target": "model", **{key: value for key, value in location.items() if key not in ("aspectRatio", "snapshotSha256")}}
                                  for location in topic["locations"]]} for topic in wanted["topics"]]}
    try:
        observed = core_values(review, wanted["version"])
        errors.extend(differences(core_expected, observed, "review"))
        return {"matched": not errors, "differences": errors, "observed": observed}
    except (KeyError, TypeError, ValueError) as error:
        return {"matched": False, "differences": [*errors, "review:" + type(error).__name__]}
