package dev.mrbbot.client;

import com.google.gwt.core.client.EntryPoint;

public class gwt implements EntryPoint {
  public void onModuleLoad() {
    onLoad();
  }

  public native void onLoad() /*-{
    $wnd.onLoad();
  }-*/;
}
