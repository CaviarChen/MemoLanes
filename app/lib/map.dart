import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/plugin_api.dart';
import 'package:mutex/mutex.dart';
import 'package:latlong2/latlong.dart';

import 'ffi.dart' if (dart.library.html) 'ffi_web.dart';

class MapUiBody extends StatefulWidget {
  const MapUiBody();

  @override
  State<StatefulWidget> createState() => MapUiBodyState();
}

class MapUiBodyState extends State<MapUiBody> {
  MapUiBodyState();

  // static const CameraPosition _kInitialPosition = CameraPosition(
  //   target: LatLng(-33.852, 151.211),
  //   zoom: 11.0,
  // );

  // bool ready = false;
  // bool layerAdded = false;
  // final m = Mutex();
  // MaplibreMapController? mapController;
  // Uint8List? image;
  LatLngBounds? overlayImageBounds;

  @override
  void initState() {
    super.initState();
  }

  // void _onMapCreated(MaplibreMapController controller) async {
  //   controller.addListener(_onMapChanged);
  //   mapController = controller;
  // }

  // void _triggerRefresh() async {
  //   // TODO: this is buggy when view is at the meridian, or when the map is
  //   // zoom out.
  //   if (!ready) return;
  //   var controller = mapController;
  //   if (controller == null) return;
  //   final zoom = controller.cameraPosition?.zoom;
  //   if (zoom == null) return;
  //   if (!zoom.isFinite) return;
  //   final visiableRegion = await controller.getVisibleRegion();
  //   final left = visiableRegion.southwest.longitude;
  //   final top = visiableRegion.northeast.latitude;
  //   final right = visiableRegion.northeast.longitude;
  //   final bottom = visiableRegion.southwest.latitude;

  //   // TODO: we use mutex to make sure only one rendering is happening at the
  //   // same time, but what we really want is: if there are multiple request
  //   // queuing up, only run the final one.
  //   await m.protect(() async {
  //     final renderResult = await api.renderMapOverlay(
  //         zoom: zoom, left: left, top: top, right: right, bottom: bottom);

  //     if (renderResult != null) {
  //       final coordinates = LatLngQuad(
  //         topLeft: LatLng(renderResult.top, renderResult.left),
  //         topRight: LatLng(renderResult.top, renderResult.right),
  //         bottomRight: LatLng(renderResult.bottom, renderResult.right),
  //         bottomLeft: LatLng(renderResult.bottom, renderResult.left),
  //       );
  //       if (layerAdded) {
  //         await mapController?.updateImageSource(
  //             "main-image-source", renderResult.data, coordinates);
  //       } else {
  //         layerAdded = true;
  //         await controller.addImageSource(
  //             "main-image-source", renderResult.data, coordinates);
  //         await controller.addImageLayer(
  //             "main-image-layer", "main-image-source");
  //       }
  //     }
  //   });
  // }

  // _onStyleLoadedCallback() async {
  //   ready = true;
  //   _triggerRefresh();
  // }

  // void _onMapChanged() async {
  //   _triggerRefresh();
  // }

  @override
  void dispose() {
    // mapController?.removeListener(_onMapChanged);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final _overlayImageBounds = overlayImageBounds;
    return FlutterMap(
      options: MapOptions(
        onPositionChanged: (position, hasGesture) {
          var bounds = position.bounds;
          if (bounds == null) return;

          setState(() {
            overlayImageBounds = bounds;
          });
          var nw = bounds.northWest;
          var se = bounds.southEast;
          print("nw: $nw, se: $se");
        },
        center: LatLng(51.509364, -0.128928),
        zoom: 9.2,
      ),
      nonRotatedChildren: [
        RichAttributionWidget(
          attributions: [
            TextSourceAttribution('OpenStreetMap contributors', onTap: () => ()
                // launchUrl(Uri.parse('https://openstreetmap.org/copyright')),
                ),
          ],
        ),
      ],
      children: [
        TileLayer(
          urlTemplate:
              'https://webrd01.is.autonavi.com/appmaptile?lang=zh_cn&size=1&scale=1&style=7&x={x}&y={y}&z={z}',
          userAgentPackageName: 'com.example.app',
        ),
        OverlayImageLayer(overlayImages: [
          if (_overlayImageBounds != null)
            OverlayImage(
              bounds: _overlayImageBounds,
              imageProvider: NetworkImage(
                  "https://img2.baidu.com/it/u=3084152093,2519646258&fm=253&fmt=auto&app=138&f=JPEG?w=500&h=839"),
              opacity: 0.5,
            ),
        ]),
      ],
    );
  }
}
